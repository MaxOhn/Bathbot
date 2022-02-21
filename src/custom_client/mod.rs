mod deserialize;
mod error;
mod osekai;
mod osu_daily;
mod osu_stats;
mod score;
mod snipe;
mod twitch;

use std::{borrow::Cow, fmt::Write, hash::Hash};

use chrono::{DateTime, Utc};
use hashbrown::HashSet;
use hyper::header::HeaderValue;
use leaky_bucket_lite::LeakyBucket;
use reqwest::{multipart::Form, Client, RequestBuilder, Response, StatusCode};
use rosu_v2::prelude::{GameMode, GameMods, User};
use serde::Serialize;
use serde_json::Value;
use tokio::time::{interval, sleep, timeout, Duration};

use crate::{
    core::BotConfig,
    util::{
        constants::{
            common_literals::{COUNTRY, MODS, SORT, USER_ID},
            AVATAR_URL, HUISMETBENEN, OSU_BASE, OSU_DAILY_API, TWITCH_OAUTH,
            TWITCH_STREAM_ENDPOINT, TWITCH_USERS_ENDPOINT, TWITCH_VIDEOS_ENDPOINT,
        },
        numbers::round,
        osu::ModSelection,
    },
    CONFIG,
};

pub use self::{error::*, osekai::*, osu_daily::*, osu_stats::*, score::*, snipe::*, twitch::*};

use self::score::ScraperScores;

type ClientResult<T> = Result<T, CustomClientError>;

static USER_AGENT: &str = env!("CARGO_PKG_NAME");

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
#[repr(u8)]
enum Site {
    Huismetbenen,
    OsuStats,
    OsuHiddenApi,
    OsuAvatar,
    Osekai,
    OsuDaily,
    Twitch,
}

pub struct CustomClient {
    client: Client,
    osu_session: &'static str,
    twitch: TwitchData,
    ratelimiters: [LeakyBucket; 7],
}

struct TwitchData {
    client_id: HeaderValue,
    oauth_token: TwitchOAuthToken,
}

impl CustomClient {
    pub async fn new(config: &'static BotConfig) -> ClientResult<Self> {
        let twitch_client_id = &config.tokens.twitch_client_id;
        let twitch_token = &config.tokens.twitch_token;

        let client = Client::builder().user_agent(USER_AGENT).build()?;

        let twitch = Self::get_twitch_token(&client, twitch_client_id, twitch_token).await?;

        // 2 per second
        let ratelimiter = || {
            LeakyBucket::builder()
                .max(2)
                .tokens(2)
                .refill_interval(Duration::from_millis(500))
                .refill_amount(1)
                .build()
        };

        // 5 per second
        let twitch_ratelimiter = LeakyBucket::builder()
            .max(5)
            .tokens(5)
            .refill_interval(Duration::from_millis(200))
            .refill_amount(1)
            .build();

        let ratelimiters = [
            ratelimiter(),      // Huismetbenen
            ratelimiter(),      // OsuStats
            ratelimiter(),      // OsuHiddenApi
            ratelimiter(),      // OsuAvatar
            ratelimiter(),      // Osekai
            ratelimiter(),      // OsuDaily
            twitch_ratelimiter, // Twitch
        ];

        Ok(Self {
            client,
            osu_session: &config.tokens.osu_session,
            twitch,
            ratelimiters,
        })
    }

    async fn get_twitch_token(
        client: &Client,
        client_id: &str,
        token: &str,
    ) -> ClientResult<TwitchData> {
        let form = &[
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", token),
        ];

        let client_id = HeaderValue::from_str(client_id)?;

        let bytes = client
            .post(TWITCH_OAUTH)
            .form(form)
            .header("Client-ID", client_id.clone())
            .send()
            .await?
            .bytes()
            .await?;

        let oauth_token = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::TwitchToken))?;

        Ok(TwitchData {
            client_id,
            oauth_token,
        })
    }

    async fn ratelimit(&self, site: Site) {
        self.ratelimiters[site as usize].acquire_one().await
    }

    async fn make_get_request(&self, url: impl AsRef<str>, site: Site) -> ClientResult<Response> {
        let url = url.as_ref();

        trace!("GET request of url {url}");
        let req = self.client.get(url);

        self.make_get_request_(req, site).await
    }

    async fn make_twitch_get_request<T: Serialize>(
        &self,
        url: impl AsRef<str>,
        data: &T,
    ) -> ClientResult<Response> {
        let url = url.as_ref();

        trace!("GET request of url {url}");
        let req = self.client.get(url).query(data);

        self.make_get_request_(req, Site::Twitch).await
    }

    async fn make_get_request_(
        &self,
        mut req: RequestBuilder,
        site: Site,
    ) -> ClientResult<Response> {
        match site {
            Site::OsuHiddenApi => {
                req = req.header("Cookie", format!("osu_session={}", self.osu_session));
            }
            Site::Twitch => {
                req = req
                    .header("Client-ID", self.twitch.client_id.clone())
                    .bearer_auth(&self.twitch.oauth_token);
            }
            _ => {}
        }

        self.ratelimit(site).await;

        Ok(req.send().await?.error_for_status()?)
    }

    async fn make_post_request<F: Serialize>(
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

        let bytes = self
            .make_post_request(url, Site::Osekai, form)
            .await?
            .bytes()
            .await?;

        let medals: OsekaiMedals = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiMedals))?;

        Ok(medals.0)
    }

    pub async fn get_osekai_beatmaps(&self, medal_name: &str) -> ClientResult<Vec<OsekaiMap>> {
        let url = "https://osekai.net/medals/api/beatmaps.php";
        let form = &[("strSearch", medal_name)];

        let bytes = self
            .make_post_request(url, Site::Osekai, form)
            .await?
            .bytes()
            .await?;

        let maps: OsekaiMaps = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiMaps))?;

        Ok(maps.0.unwrap_or_default())
    }

    pub async fn get_osekai_comments(&self, medal_name: &str) -> ClientResult<Vec<OsekaiComment>> {
        let url = "https://osekai.net/global/api/comment_system.php";
        let form = &[("strMedalName", medal_name), ("bGetComments", "true")];

        let bytes = self
            .make_post_request(url, Site::Osekai, form)
            .await?
            .bytes()
            .await?;

        let comments: OsekaiComments = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiComments))?;

        Ok(comments.0.unwrap_or_default())
    }

    pub async fn get_osekai_ranking<R: OsekaiRanking>(&self) -> ClientResult<Vec<R::Entry>> {
        let url = "https://osekai.net/rankings/api/api.php";
        let form = &[("App", R::FORM)];

        let bytes = self
            .make_post_request(url, Site::Osekai, form)
            .await?
            .bytes()
            .await?;

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

        let bytes = self
            .make_get_request(url, Site::Huismetbenen)
            .await?
            .bytes()
            .await?;

        let player: SnipePlayer = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipePlayer))?;

        Ok(player)
    }

    pub async fn get_snipe_country(&self, country: &str) -> ClientResult<Vec<SnipeCountryPlayer>> {
        let url = format!(
            "{HUISMETBENEN}rankings/{}/pp/weighted",
            country.to_lowercase()
        );

        let bytes = self
            .make_get_request(url, Site::Huismetbenen)
            .await?
            .bytes()
            .await?;

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

        let bytes = self
            .make_get_request(url, Site::Huismetbenen)
            .await?
            .bytes()
            .await?;

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

        let bytes = self
            .make_get_request(url, Site::Huismetbenen)
            .await?
            .bytes()
            .await?;

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

        let bytes = self
            .make_get_request(url, Site::Huismetbenen)
            .await?
            .bytes()
            .await?;

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

        let bytes = self
            .make_get_request(url, Site::Huismetbenen)
            .await?
            .bytes()
            .await?;

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

        let bytes = match timeout(Duration::from_secs(4), request.send()).await {
            Ok(result) => result?.bytes().await?,
            Err(_) => return Err(CustomClientError::OsuStatsTimeout),
        };

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

        let bytes = match timeout(Duration::from_secs(4), request.send()).await {
            Ok(result) => result?.bytes().await?,
            Err(_) => return Err(CustomClientError::OsuStatsTimeout),
        };

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

        let bytes = self
            .make_get_request(url, Site::OsuHiddenApi)
            .await?
            .bytes()
            .await?;

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

        let bytes = loop {
            let response = self.make_get_request(&url, Site::OsuDaily).await?;

            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                break response.bytes().await?;
            }

            debug!("Ratelimited by osudaily, wait a second");
            sleep(Duration::from_secs(1)).await;
        };

        let rank_pp = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::RankData))?;

        Ok(rank_pp)
    }

    pub async fn get_twitch_user(&self, name: &str) -> ClientResult<Option<TwitchUser>> {
        let data = [("login", name)];

        let bytes = self
            .make_twitch_get_request(TWITCH_USERS_ENDPOINT, &data)
            .await?
            .bytes()
            .await?;

        let mut users: TwitchDataList<TwitchUser> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::TwitchUserName))?;

        Ok(users.data.pop())
    }

    pub async fn get_twitch_user_by_id(&self, user_id: u64) -> ClientResult<Option<TwitchUser>> {
        let data = [("id", user_id)];

        let bytes = self
            .make_twitch_get_request(TWITCH_USERS_ENDPOINT, &data)
            .await?
            .bytes()
            .await?;

        let mut users: TwitchDataList<TwitchUser> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::TwitchUserId))?;

        Ok(users.data.pop())
    }

    pub async fn get_twitch_users(&self, user_ids: &[u64]) -> ClientResult<Vec<TwitchUser>> {
        let mut users = Vec::with_capacity(user_ids.len());

        for chunk in user_ids.chunks(100) {
            let data: Vec<_> = chunk.iter().map(|&id| ("id", id)).collect();

            let bytes = self
                .make_twitch_get_request(TWITCH_USERS_ENDPOINT, &data)
                .await?
                .bytes()
                .await?;

            let parsed_response: TwitchDataList<TwitchUser> = serde_json::from_slice(&bytes)
                .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::TwitchUsers))?;

            users.extend(parsed_response.data);
        }

        Ok(users)
    }

    pub async fn get_twitch_streams(&self, user_ids: &[u64]) -> ClientResult<Vec<TwitchStream>> {
        let mut streams = Vec::with_capacity(user_ids.len());
        let mut interval = interval(Duration::from_millis(1000));

        for chunk in user_ids.chunks(100) {
            interval.tick().await;
            let mut data: Vec<_> = chunk.iter().map(|&id| (USER_ID, id)).collect();
            data.push(("first", chunk.len() as u64));

            let bytes = self
                .make_twitch_get_request(TWITCH_STREAM_ENDPOINT, &data)
                .await?
                .bytes()
                .await?;

            let parsed_response: TwitchDataList<TwitchStream> = serde_json::from_slice(&bytes)
                .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::TwitchStreams))?;

            streams.extend(parsed_response.data);
        }

        Ok(streams)
    }

    pub async fn get_last_twitch_vod(&self, user_id: u64) -> ClientResult<Option<TwitchVideo>> {
        let data = [
            (USER_ID, Cow::Owned(user_id.to_string())),
            ("first", "1".into()),
            (SORT, "time".into()),
        ];

        let bytes = self
            .make_twitch_get_request(TWITCH_VIDEOS_ENDPOINT, &data)
            .await?
            .bytes()
            .await?;

        let mut videos: TwitchDataList<TwitchVideo> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::TwitchVideos))?;

        Ok(videos.data.pop())
    }
}

pub enum RankParam {
    Rank(usize),
    Pp(f32),
}
