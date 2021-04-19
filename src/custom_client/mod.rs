mod deserialize;
mod osekai;
mod osu_daily;
mod osu_stats;
mod score;
mod snipe;

pub use osekai::*;
pub use osu_daily::*;
pub use osu_stats::*;
use score::ScraperScores;
pub use score::{ScraperBeatmap, ScraperScore};
pub use snipe::*;

use crate::{
    util::{
        constants::{AVATAR_URL, HUISMETBENEN, OSEKAI_MEDAL_API, OSU_BASE, OSU_DAILY_API},
        error::CustomClientError,
        numbers::round,
        osu::ModSelection,
    },
    BotResult, CONFIG,
};

use chrono::{DateTime, Utc};
use governor::{clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter};
use hashbrown::HashSet;
use once_cell::sync::OnceCell;
use reqwest::{multipart::Form, Client, Response, StatusCode};
use rosu_v2::prelude::{GameMode, GameMods, User};
use serde_json::Value;
use std::{fmt::Write, hash::Hash, num::NonZeroU32};
use tokio::time::{sleep, timeout, Duration};

type ClientResult<T> = Result<T, CustomClientError>;

static USER_AGENT: &str = env!("CARGO_PKG_NAME");
static OSU_SESSION: OnceCell<&'static str> = OnceCell::new();

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
enum Site {
    OsuStats,
    OsuHiddenApi,
    OsuAvatar,
    OsuSnipe,
    Osekai,
    OsuDaily,
}

pub struct CustomClient {
    client: Client,
    ratelimiter: RateLimiter<Site, DashMapStateStore<Site>, DefaultClock>,
}

impl CustomClient {
    pub async fn new() -> BotResult<Self> {
        let config = CONFIG.get().unwrap();

        OSU_SESSION.set(&config.tokens.osu_session).unwrap();

        let client = Client::builder().user_agent(USER_AGENT).build()?;
        let quota = Quota::per_second(NonZeroU32::new(2).unwrap());
        let ratelimiter = RateLimiter::dashmap_with_clock(quota, &DefaultClock::default());

        Ok(Self {
            client,
            ratelimiter,
        })
    }

    #[inline]
    async fn ratelimit(&self, site: Site) {
        self.ratelimiter.until_key_ready(&site).await
    }

    async fn make_request(&self, url: impl AsRef<str>, site: Site) -> ClientResult<Response> {
        let url = url.as_ref();

        debug!("Requesting url {}", url);
        let mut req = self.client.get(url);

        if let Site::OsuHiddenApi = site {
            req = req.header(
                "Cookie",
                format!("osu_session={}", OSU_SESSION.get().unwrap()),
            )
        }

        self.ratelimit(site).await;

        Ok(req.send().await?.error_for_status()?)
    }

    pub async fn get_osekai_medal(&self, medal_name: &str) -> ClientResult<Option<OsekaiMedal>> {
        let url = format!("{}get_medal?medal={}", OSEKAI_MEDAL_API, medal_name);
        let response = self.make_request(url, Site::Osekai).await?;
        let bytes = response.bytes().await?;

        let medal: Option<OsekaiMedal> = match serde_json::from_slice(&bytes) {
            Ok(medal) => Some(medal),
            Err(source) => match serde_json::from_slice::<Value>(&bytes)
                .map(|mut v| v.get_mut("error").map(Value::take))
            {
                Ok(Some(Value::String(msg))) if msg == "Medal could not be found" => None,
                _ => {
                    return Err(CustomClientError::Parsing {
                        body: String::from_utf8_lossy(&bytes).into_owned(),
                        source,
                        request: "osekai medal",
                    })
                }
            },
        };

        Ok(medal)
    }

    pub async fn get_snipe_player(&self, country: &str, user_id: u32) -> ClientResult<SnipePlayer> {
        let url = format!(
            "{}player/{}/{}?type=id",
            HUISMETBENEN,
            country.to_lowercase(),
            user_id
        );

        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;

        let player: SnipePlayer =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "snipe player",
            })?;

        Ok(player)
    }

    pub async fn get_snipe_country(&self, country: &str) -> ClientResult<Vec<SnipeCountryPlayer>> {
        let url = format!(
            "{}rankings/{}/pp/weighted",
            HUISMETBENEN,
            country.to_lowercase()
        );

        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;

        let country_players: Vec<SnipeCountryPlayer> =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "snipe country",
            })?;

        Ok(country_players)
    }

    pub async fn get_country_statistics(
        &self,
        country: &str,
    ) -> ClientResult<SnipeCountryStatistics> {
        let country = country.to_lowercase();
        let url = format!("{}rankings/{}/statistics", HUISMETBENEN, country);

        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;

        let statistics =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "country statistics",
            })?;

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
            "{}snipes/{}/{}?since={}&until={}",
            HUISMETBENEN,
            user.user_id,
            if sniper { "new" } else { "old" },
            from.format(date_format),
            until.format(date_format)
        );

        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;

        let snipes: Vec<SnipeRecent> =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "snipe recent",
            })?;

        Ok(snipes)
    }

    pub async fn get_national_firsts(
        &self,
        params: &SnipeScoreParams,
    ) -> ClientResult<Vec<SnipeScore>> {
        let mut url = format!(
            "{base}player/{country}/{user}/topranks?page={page}&mode={mode}&sort={sort}&order={order}",
            base = HUISMETBENEN,
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
                    let _ = write!(url, "&mods={}", mods);
                }
            }
        }

        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;

        let scores: Vec<SnipeScore> =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "snipe score",
            })?;

        Ok(scores)
    }

    pub async fn get_national_firsts_count(
        &self,
        params: &SnipeScoreParams,
    ) -> ClientResult<usize> {
        let mut url = format!(
            "{base}player/{country}/{user}/topranks/count?mode={mode}",
            base = HUISMETBENEN,
            country = params.country,
            user = params.user_id,
            mode = params.mode,
        );

        if let Some(mods) = params.mods {
            if let ModSelection::Include(mods) | ModSelection::Exact(mods) = mods {
                if mods == GameMods::NoMod {
                    url.push_str("&mods=nomod");
                } else {
                    let _ = write!(url, "&mods={}", mods);
                }
            }
        }

        let response = self.make_request(url, Site::OsuSnipe).await?;
        let bytes = response.bytes().await?;

        let count: usize =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "snipe score count",
            })?;

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
            form = form.text("country", country.to_string());
        }

        let url = "https://osustats.ppy.sh/api/getScoreRanking";
        debug!("Requesting POST from url {} [page {}]", url, params.page);
        let request = self.client.post(url).multipart(form);
        self.ratelimit(Site::OsuStats).await;

        let response = match timeout(Duration::from_secs(4), request.send()).await {
            Ok(result) => result?,
            Err(_) => return Err(CustomClientError::OsuStatsTimeout),
        };

        let bytes = response.bytes().await?;

        let players: Vec<OsuStatsPlayer> =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "globals list",
            })?;

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
                ModSelection::Include(mods) => write!(mod_str, "+{}", mods),
                ModSelection::Exclude(mods) => write!(mod_str, "-{}", mods),
                ModSelection::Exact(mods) => write!(mod_str, "!{}", mods),
            };

            form = form.text("mods", mod_str);
        }

        let url = "https://osustats.ppy.sh/api/getScores";
        debug!("Requesting POST from url {}", url);
        let request = self.client.post(url).multipart(form);
        self.ratelimit(Site::OsuStats).await;

        let response = match timeout(Duration::from_secs(4), request.send()).await {
            Ok(result) => result?,
            Err(_) => return Err(CustomClientError::OsuStatsTimeout),
        };

        let bytes = response.bytes().await?;

        let result: Value =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "osu stats global",
            })?;

        let (scores, amount) = if let Value::Array(mut array) = result {
            let mut values = array.drain(..2);

            let scores = serde_json::from_value(values.next().unwrap()).map_err(|source| {
                CustomClientError::Parsing {
                    body: String::from_utf8_lossy(&bytes).into_owned(),
                    source,
                    request: "osu stats global scores",
                }
            })?;

            let amount = serde_json::from_value(values.next().unwrap()).map_err(|source| {
                CustomClientError::Parsing {
                    body: String::from_utf8_lossy(&bytes).into_owned(),
                    source,
                    request: "osu stats global amount",
                }
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

        // Check another request for mania's MR is needed
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
                    let _ = write!(url, "&mods[]={}", m);
                }
            }
        }

        let response = self.make_request(url, Site::OsuHiddenApi).await?;
        let bytes = response.bytes().await?;

        let scores: ScraperScores =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "leaderboard",
            })?;

        Ok(scores.get())
    }

    pub async fn get_avatar(&self, user_id: u32) -> ClientResult<Vec<u8>> {
        let url = format!("{}{}", AVATAR_URL, user_id);
        let response = self.make_request(url, Site::OsuAvatar).await?;

        Ok(response.bytes().await?.to_vec())
    }

    pub async fn get_rank_data(&self, mode: GameMode, param: RankParam) -> ClientResult<RankPP> {
        let key = &CONFIG.get().unwrap().tokens.osu_daily;
        let mut url = format!("{}pp.php?k={}&m={}&", OSU_DAILY_API, key, mode as u8);

        let _ = match param {
            RankParam::Rank(rank) => write!(url, "t=rank&v={}", rank),
            RankParam::Pp(pp) => write!(url, "t=pp&v={}", round(pp)),
        };

        const SECOND: Duration = Duration::from_secs(1);

        let response = loop {
            let response = self.make_request(&url, Site::OsuDaily).await?;

            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                break response;
            }

            debug!("Ratelimited by osudaily, wait a second");
            sleep(SECOND).await;
        };

        let bytes = response.bytes().await?;

        let rank_pp =
            serde_json::from_slice(&bytes).map_err(|source| CustomClientError::Parsing {
                body: String::from_utf8_lossy(&bytes).into_owned(),
                source,
                request: "rank data",
            })?;

        Ok(rank_pp)
    }
}

pub enum RankParam {
    Rank(usize),
    Pp(f32),
}
