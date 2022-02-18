mod deserialize;
mod error;
mod osekai;
mod osu_daily;
mod osu_stats;
mod score;
mod snipe;

use std::{fmt::Write, hash::Hash};

use chrono::{DateTime, Utc};
use hashbrown::HashSet;
use leaky_bucket_lite::LeakyBucket;
use once_cell::sync::OnceCell;
use reqwest::{multipart::Form, Client, Response, StatusCode};
use rosu_v2::prelude::{GameMode, GameMods, User};
use serde::Serialize;
use serde_json::Value;
use tokio::time::{sleep, timeout, Duration};

use crate::{
    util::{
        constants::{
            common_literals::{COUNTRY, MODS},
            AVATAR_URL, HUISMETBENEN, OSU_BASE, OSU_DAILY_API,
        },
        numbers::round,
        osu::ModSelection,
    },
    BotResult, CONFIG,
};

pub use self::{
    error::CustomClientError,
    osekai::*,
    osu_daily::*,
    osu_stats::*,
    score::{ScraperBeatmap, ScraperScore},
    snipe::*,
};

use self::{error::ErrorKind, score::ScraperScores};

type ClientResult<T> = Result<T, CustomClientError>;

static USER_AGENT: &str = env!("CARGO_PKG_NAME");
static OSU_SESSION: OnceCell<&'static str> = OnceCell::new();

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
#[repr(u8)]
enum Site {
    Huismetbenen,
    OsuStats,
    OsuHiddenApi,
    OsuAvatar,
    Osekai,
    OsuDaily,
}

pub struct CustomClient {
    client: Client,
    ratelimiters: [LeakyBucket; 6],
}

impl CustomClient {
    pub async fn new() -> BotResult<Self> {
        let config = CONFIG.get().unwrap();

        OSU_SESSION.set(&config.tokens.osu_session).unwrap();

        let client = Client::builder().user_agent(USER_AGENT).build()?;

        let ratelimiter = || {
            LeakyBucket::builder()
                .max(2)
                .tokens(2)
                .refill_interval(Duration::from_millis(500))
                .refill_amount(1)
                .build()
        };

        let ratelimiters = [
            ratelimiter(), // Huismetbenen
            ratelimiter(), // OsuStats
            ratelimiter(), // OsuHiddenApi
            ratelimiter(), // OsuAvatar
            ratelimiter(), // Osekai
            ratelimiter(), // OsuDaily
        ];

        Ok(Self {
            client,
            ratelimiters,
        })
    }

    async fn ratelimit(&self, site: Site) {
        self.ratelimiters[site as usize].acquire_one().await
    }

    async fn make_get_request(&self, url: impl AsRef<str>, site: Site) -> ClientResult<Response> {
        let url = url.as_ref();

        trace!("GET request of url {url}");
        let mut req = self.client.get(url);

        if let Site::OsuHiddenApi = site {
            let cookie = format!("osu_session={}", OSU_SESSION.get().unwrap());
            req = req.header("Cookie", cookie)
        }

        self.ratelimit(site).await;

        Ok(req.send().await?.error_for_status()?)
    }

    async fn make_post_request<F: Serialize + ?Sized>(
        &self,
        url: impl AsRef<str>,
        site: Site,
        form: &F,
    ) -> ClientResult<Response> {
        let url = url.as_ref();

        trace!("POST request of url {url}");
        let req = self.client.post(url).form(form);
        self.ratelimit(site).await;

        Ok(req.send().await?.error_for_status()?)
    }

    pub async fn get_osekai_medals(&self) -> ClientResult<Vec<OsekaiMedal>> {
        let url = "https://osekai.net/medals/api/medals.php";
        let form = &[("strSearch", "")];

        let response = self.make_post_request(url, Site::Osekai, form).await?;
        let bytes = response.bytes().await?;

        let medals: OsekaiMedals = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiMedals))?;

        Ok(medals.0)
    }

    pub async fn get_osekai_beatmaps(&self, medal_name: &str) -> ClientResult<Vec<OsekaiMap>> {
        let url = "https://osekai.net/medals/api/beatmaps.php";
        let form = &[("strSearch", medal_name)];

        let response = self.make_post_request(url, Site::Osekai, form).await?;
        let bytes = response.bytes().await?;

        let maps: OsekaiMaps = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiMaps))?;

        Ok(maps.0.unwrap_or_default())
    }

    pub async fn get_osekai_comments(&self, medal_name: &str) -> ClientResult<Vec<OsekaiComment>> {
        let url = "https://osekai.net/global/api/comment_system.php";
        let form = &[("strMedalName", medal_name), ("bGetComments", "true")];

        let response = self.make_post_request(url, Site::Osekai, form).await?;
        let bytes = response.bytes().await?;

        let comments: OsekaiComments = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiComments))?;

        Ok(comments.0.unwrap_or_default())
    }

    pub async fn get_osekai_ranking<R: OsekaiRanking>(&self) -> ClientResult<Vec<R::Entry>> {
        let url = "https://osekai.net/rankings/api/api.php";
        let form = &[("App", R::FORM)];

        let response = self.make_post_request(url, Site::Osekai, form).await?;
        let bytes = response.bytes().await?;

        let ranking = serde_json::from_slice(&bytes).map_err(|e| {
            CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiRanking(R::REQUEST))
        })?;

        Ok(ranking)
    }

    pub async fn get_snipe_player(&self, country: &str, user_id: u32) -> ClientResult<SnipePlayer> {
        let url = format!(
            "{HUISMETBENEN}player/{}/{user_id}?type=id",
            country.to_lowercase(),
        );

        let response = self.make_get_request(url, Site::Huismetbenen).await?;
        let bytes = response.bytes().await?;

        let player: SnipePlayer = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipePlayer))?;

        Ok(player)
    }

    pub async fn get_snipe_country(&self, country: &str) -> ClientResult<Vec<SnipeCountryPlayer>> {
        let url = format!(
            "{HUISMETBENEN}rankings/{}/pp/weighted",
            country.to_lowercase()
        );

        let response = self.make_get_request(url, Site::Huismetbenen).await?;
        let bytes = response.bytes().await?;

        let country_players: Vec<SnipeCountryPlayer> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipeCountry))?;

        Ok(country_players)
    }

    pub async fn get_country_statistics(
        &self,
        country: &str,
    ) -> ClientResult<SnipeCountryStatistics> {
        let country = country.to_lowercase();
        let url = format!("{HUISMETBENEN}rankings/{country}/statistics");

        let response = self.make_get_request(url, Site::Huismetbenen).await?;
        let bytes = response.bytes().await?;

        let statistics = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::CountryStatistics))?;

        Ok(statistics)
    }

    pub async fn get_national_snipes(
        &self,
        user: &User,
        sniper: bool,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> ClientResult<Vec<SnipeRecent>> {
        let date_format = "%FT%TZ";

        let url = format!(
            "{HUISMETBENEN}snipes/{}/{}?since={}&until={}",
            user.user_id,
            if sniper { "new" } else { "old" },
            from.format(date_format),
            until.format(date_format)
        );

        let response = self.make_get_request(url, Site::Huismetbenen).await?;
        let bytes = response.bytes().await?;

        let snipes: Vec<SnipeRecent> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipeRecent))?;

        Ok(snipes)
    }

    pub async fn get_national_firsts(
        &self,
        params: &SnipeScoreParams,
    ) -> ClientResult<Vec<SnipeScore>> {
        let mut url = format!(
            "{HUISMETBENEN}player/{country}/{user}/topranks?page={page}&mode={mode}&sort={sort}&order={order}",
            country = params.country,
            user = params.user_id,
            page = params.page,
            mode = params.mode,
            sort = params.order,
            order = if params.descending { "desc" } else { "asc" },
        );

        if let Some(mods) = params.mods {
            if let ModSelection::Include(mods) | ModSelection::Exact(mods) = mods {
                if mods == GameMods::NoMod {
                    url.push_str("&mods=nomod");
                } else {
                    let _ = write!(url, "&mods={mods}");
                }
            }
        }

        let response = self.make_get_request(url, Site::Huismetbenen).await?;
        let bytes = response.bytes().await?;

        let scores: Vec<SnipeScore> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipeScore))?;

        Ok(scores)
    }

    pub async fn get_national_firsts_count(
        &self,
        params: &SnipeScoreParams,
    ) -> ClientResult<usize> {
        let mut url = format!(
            "{HUISMETBENEN}player/{country}/{user}/topranks/count?mode={mode}",
            country = params.country,
            user = params.user_id,
            mode = params.mode,
        );

        if let Some(mods) = params.mods {
            if let ModSelection::Include(mods) | ModSelection::Exact(mods) = mods {
                if mods == GameMods::NoMod {
                    url.push_str("&mods=nomod");
                } else {
                    let _ = write!(url, "&mods={mods}");
                }
            }
        }

        let response = self.make_get_request(url, Site::Huismetbenen).await?;
        let bytes = response.bytes().await?;

        let count: usize = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipeScoreCount))?;

        Ok(count)
    }

    pub async fn get_country_globals(
        &self,
        params: &OsuStatsListParams,
    ) -> ClientResult<Vec<OsuStatsPlayer>> {
        let mut form = Form::new()
            .text("rankMin", params.rank_min.to_string())
            .text("rankMax", params.rank_max.to_string())
            .text("gamemode", (params.mode as u8).to_string())
            .text("page", params.page.to_string());

        if let Some(ref country) = params.country {
            form = form.text(COUNTRY, country.to_string());
        }

        let url = "https://osustats.ppy.sh/api/getScoreRanking";
        trace!("Requesting POST from url {url} [page {}]", params.page);
        let request = self.client.post(url).multipart(form);
        self.ratelimit(Site::OsuStats).await;

        let response = match timeout(Duration::from_secs(4), request.send()).await {
            Ok(result) => result?,
            Err(_) => return Err(CustomClientError::OsuStatsTimeout),
        };

        let bytes = response.bytes().await?;

        let players: Vec<OsuStatsPlayer> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::GlobalsList))?;

        Ok(players)
    }

    /// Be sure whitespaces in the username are **not** replaced
    pub async fn get_global_scores(
        &self,
        params: &OsuStatsParams,
    ) -> ClientResult<(Vec<OsuStatsScore>, usize)> {
        let mut form = Form::new()
            .text("accMin", params.acc_min.to_string())
            .text("accMax", params.acc_max.to_string())
            .text("rankMin", params.rank_min.to_string())
            .text("rankMax", params.rank_max.to_string())
            .text("gamemode", (params.mode as u8).to_string())
            .text("sortBy", (params.order as u8).to_string())
            .text("sortOrder", (!params.descending as u8).to_string())
            .text("page", params.page.to_string())
            .text("u1", params.username.clone().into_string());

        if let Some(selection) = params.mods {
            let mut mod_str = String::with_capacity(3);

            let _ = match selection {
                ModSelection::Include(mods) => write!(mod_str, "+{mods}"),
                ModSelection::Exclude(mods) => write!(mod_str, "-{mods}"),
                ModSelection::Exact(mods) => write!(mod_str, "!{mods}"),
            };

            form = form.text(MODS, mod_str);
        }

        let url = "https://osustats.ppy.sh/api/getScores";
        trace!("Requesting POST from url {url}");
        let request = self.client.post(url).multipart(form);
        self.ratelimit(Site::OsuStats).await;

        let response = match timeout(Duration::from_secs(4), request.send()).await {
            Ok(result) => result?,
            Err(_) => return Err(CustomClientError::OsuStatsTimeout),
        };

        let bytes = response.bytes().await?;

        let result: Value = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsuStatsGlobal))?;

        let (scores, amount) = if let Value::Array(mut array) = result {
            let mut values = array.drain(..2);

            let scores = serde_json::from_value(values.next().unwrap()).map_err(|e| {
                CustomClientError::parsing(e, &bytes, ErrorKind::OsuStatsGlobalScores)
            })?;

            let amount = serde_json::from_value(values.next().unwrap()).map_err(|e| {
                CustomClientError::parsing(e, &bytes, ErrorKind::OsuStatsGlobalAmount)
            })?;

            (scores, amount)
        } else {
            (Vec::new(), 0)
        };

        Ok((scores, amount))
    }

    // Retrieve the leaderboard of a map (national / global)
    // If mods contain DT / NC, it will do another request for the opposite
    // If mods dont contain Mirror and its a mania map, it will perform the
    // same requests again but with Mirror enabled
    pub async fn get_leaderboard(
        &self,
        map_id: u32,
        national: bool,
        mods: Option<GameMods>,
        mode: GameMode,
    ) -> ClientResult<Vec<ScraperScore>> {
        let mut scores = self._get_leaderboard(map_id, national, mods).await?;

        let non_mirror = mods
            .map(|mods| !mods.contains(GameMods::Mirror))
            .unwrap_or(true);

        // Check if another request for mania's MR is needed
        if mode == GameMode::MNA && non_mirror {
            let mods = match mods {
                None => Some(GameMods::Mirror),
                Some(mods) => Some(mods | GameMods::Mirror),
            };

            let mut new_scores = self._get_leaderboard(map_id, national, mods).await?;
            scores.append(&mut new_scores);
            scores.sort_unstable_by(|a, b| b.score.cmp(&a.score));
            let mut uniques = HashSet::with_capacity(50);
            scores.retain(|s| uniques.insert(s.user_id));
            scores.truncate(50);
        }

        // Check if DT / NC is included
        let mods = match mods {
            Some(mods) if mods.contains(GameMods::DoubleTime) => Some(mods | GameMods::NightCore),
            Some(mods) if mods.contains(GameMods::NightCore) => {
                Some((mods - GameMods::NightCore) | GameMods::DoubleTime)
            }
            Some(_) | None => None,
        };

        // If DT / NC included, make another request
        if mods.is_some() {
            if mode == GameMode::MNA && non_mirror {
                let mods = mods.map(|mods| mods | GameMods::Mirror);
                let mut new_scores = self._get_leaderboard(map_id, national, mods).await?;
                scores.append(&mut new_scores);
            }

            let mut new_scores = self._get_leaderboard(map_id, national, mods).await?;
            scores.append(&mut new_scores);
            scores.sort_unstable_by(|a, b| b.score.cmp(&a.score));
            let mut uniques = HashSet::with_capacity(50);
            scores.retain(|s| uniques.insert(s.user_id));
            scores.truncate(50);
        }

        Ok(scores)
    }

    // Retrieve the leaderboard of a map (national / global)
    async fn _get_leaderboard(
        &self,
        map_id: u32,
        national: bool,
        mods: Option<GameMods>,
    ) -> ClientResult<Vec<ScraperScore>> {
        let mut url = format!("{base}beatmaps/{id}/scores?", base = OSU_BASE, id = map_id);

        if national {
            url.push_str("type=country");
        }

        if let Some(mods) = mods {
            if mods.is_empty() {
                url.push_str("&mods[]=NM");
            } else {
                for m in mods.iter() {
                    let _ = write!(url, "&mods[]={m}");
                }
            }
        }

        let response = self.make_get_request(url, Site::OsuHiddenApi).await?;
        let bytes = response.bytes().await?;

        let scores: ScraperScores = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::Leaderboard))?;

        Ok(scores.get())
    }

    #[allow(dead_code)]
    pub async fn get_avatar_with_id(&self, user_id: u32) -> ClientResult<Vec<u8>> {
        let url = format!("{AVATAR_URL}{user_id}");

        self.get_avatar(url).await
    }

    pub async fn get_avatar(&self, url: impl AsRef<str>) -> ClientResult<Vec<u8>> {
        let response = self.make_get_request(url, Site::OsuAvatar).await?;

        Ok(response.bytes().await?.to_vec())
    }

    pub async fn get_rank_data(&self, mode: GameMode, param: RankParam) -> ClientResult<RankPP> {
        let key = &CONFIG.get().unwrap().tokens.osu_daily;
        let mut url = format!("{OSU_DAILY_API}pp.php?k={key}&m={}&", mode as u8);

        let _ = match param {
            RankParam::Rank(rank) => write!(url, "t=rank&v={rank}"),
            RankParam::Pp(pp) => write!(url, "t=pp&v={}", round(pp)),
        };

        let response = loop {
            let response = self.make_get_request(&url, Site::OsuDaily).await?;

            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                break response;
            }

            debug!("Ratelimited by osudaily, wait a second");
            sleep(Duration::from_secs(1)).await;
        };

        let bytes = response.bytes().await?;

        let rank_pp = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::RankData))?;

        Ok(rank_pp)
    }
}

pub enum RankParam {
    Rank(usize),
    Pp(f32),
}
