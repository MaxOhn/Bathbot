mod deserialize;
mod error;
mod osekai;
mod osu_daily;
mod osu_stats;
mod score;
mod snipe;
mod twitch;

use std::{
    borrow::Cow,
    fmt::{Display, Write},
    hash::Hash,
};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use hashbrown::HashSet;
use http::{
    header::{AUTHORIZATION, CONTENT_LENGTH, COOKIE},
    request::Builder as RequestBuilder,
    Response, StatusCode,
};
use hyper::{
    client::{connect::dns::GaiResolver, Client as HyperClient, HttpConnector},
    header::{HeaderValue, CONTENT_TYPE, USER_AGENT as HYPER_USER_AGENT},
    Body, Method, Request,
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use leaky_bucket_lite::LeakyBucket;
use rosu_v2::prelude::{BeatmapsetCovers, GameMode, GameMods, User};
use serde::Serialize;
use serde_json::{Map, Value};
use tokio::time::{interval, sleep, timeout, Duration};
use twilight_model::channel::Attachment;

use crate::{
    core::BotConfig,
    util::{
        constants::{
            common_literals::{COUNTRY, MODS, SORT, USER_ID},
            HUISMETBENEN, OSU_BASE, OSU_DAILY_API, TWITCH_OAUTH, TWITCH_STREAM_ENDPOINT,
            TWITCH_USERS_ENDPOINT, TWITCH_VIDEOS_ENDPOINT,
        },
        numbers::round,
        osu::ModSelection,
        ExponentialBackoff,
    },
    CONFIG,
};

pub use self::{error::*, osekai::*, osu_daily::*, osu_stats::*, score::*, snipe::*, twitch::*};

use self::score::ScraperScores;

type ClientResult<T> = Result<T, CustomClientError>;

static MY_USER_AGENT: &str = env!("CARGO_PKG_NAME");

const APPLICATION_JSON: &str = "application/json";
const APPLICATION_URLENCODED: &str = "application/x-www-form-urlencoded";

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
#[repr(u8)]
enum Site {
    DiscordAttachment,
    Huismetbenen,
    Osekai,
    OsuAvatar,
    OsuBadge,
    OsuDaily,
    OsuHiddenApi,
    OsuMapFile,
    OsuMapsetCover,
    OsuStats,
    Twitch,
}

type Client = HyperClient<HttpsConnector<HttpConnector<GaiResolver>>, Body>;

pub struct CustomClient {
    client: Client,
    osu_session: &'static str,
    twitch: TwitchData,
    ratelimiters: [LeakyBucket; 11],
}

struct TwitchData {
    client_id: HeaderValue,
    oauth_token: TwitchOAuthToken,
}

impl CustomClient {
    pub async fn new(config: &'static BotConfig) -> ClientResult<Self> {
        let twitch_client_id = &config.tokens.twitch_client_id;
        let twitch_token = &config.tokens.twitch_token;

        let connector = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .build();

        let client = HyperClient::builder().build(connector);

        let twitch = Self::get_twitch_token(&client, twitch_client_id, twitch_token).await?;

        let ratelimiter = |per_second| {
            LeakyBucket::builder()
                .max(per_second)
                .tokens(per_second)
                .refill_interval(Duration::from_millis(1000 / per_second as u64))
                .refill_amount(1)
                .build()
        };

        let ratelimiters = [
            ratelimiter(2),  // DiscordAttachment
            ratelimiter(2),  // Huismetbenen
            ratelimiter(2),  // Osekai
            ratelimiter(10), // OsuAvatar
            ratelimiter(10), // OsuBadge
            ratelimiter(2),  // OsuDaily
            ratelimiter(2),  // OsuHiddenApi
            ratelimiter(5),  // OsuMapFile
            ratelimiter(10), // OsuMapsetCover
            ratelimiter(2),  // OsuStats
            ratelimiter(5),  // Twitch
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

        let form_body = serde_urlencoded::to_string(form)?;
        let client_id = HeaderValue::from_str(client_id)?;

        let req = Request::builder()
            .method(Method::POST)
            .uri(TWITCH_OAUTH)
            .header("Client-ID", client_id.clone())
            .header(HYPER_USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form_body))?;

        let response = client.request(req).await?;
        let bytes = Self::error_for_status(response, TWITCH_OAUTH).await?;

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

    async fn make_get_request(&self, url: impl AsRef<str>, site: Site) -> ClientResult<Bytes> {
        trace!("GET request of url {}", url.as_ref());

        let req = self
            .make_get_request_(url.as_ref(), site)
            .body(Body::empty())?;

        self.ratelimit(site).await;
        let response = self.client.request(req).await?;

        Self::error_for_status(response, url.as_ref()).await
    }

    async fn make_twitch_get_request<I, U, V>(
        &self,
        url: impl AsRef<str>,
        data: I,
    ) -> ClientResult<Bytes>
    where
        I: IntoIterator<Item = (U, V)>,
        U: Display,
        V: Display,
    {
        trace!("GET request of url {}", url.as_ref());

        let mut uri = format!("{}?", url.as_ref());
        let mut iter = data.into_iter();

        if let Some((key, value)) = iter.next() {
            let _ = write!(uri, "{key}={value}");

            for (key, value) in iter {
                let _ = write!(uri, "&{key}={value}");
            }
        }

        let req = self
            .make_get_request_(uri, Site::Twitch)
            .body(Body::empty())?;

        self.ratelimit(Site::Twitch).await;
        let response = self.client.request(req).await?;

        Self::error_for_status(response, url.as_ref()).await
    }

    fn make_get_request_(&self, url: impl AsRef<str>, site: Site) -> RequestBuilder {
        let req = Request::builder()
            .uri(url.as_ref())
            .method(Method::GET)
            .header(HYPER_USER_AGENT, MY_USER_AGENT);

        match site {
            Site::OsuHiddenApi => req.header(COOKIE, format!("osu_session={}", self.osu_session)),
            Site::Twitch => req
                .header("Client-ID", self.twitch.client_id.clone())
                .header(AUTHORIZATION, format!("Bearer {}", self.twitch.oauth_token)),
            _ => req,
        }
    }

    async fn make_post_request<F: Serialize>(
        &self,
        url: impl AsRef<str>,
        site: Site,
        form: &F,
    ) -> ClientResult<Bytes> {
        trace!("POST request of url {}", url.as_ref());

        let form_body = serde_urlencoded::to_string(form)?;

        let req = Request::builder()
            .method(Method::POST)
            .uri(url.as_ref())
            .header(HYPER_USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, APPLICATION_URLENCODED)
            .body(Body::from(form_body))?;

        self.ratelimit(site).await;
        let response = self.client.request(req).await?;

        Self::error_for_status(response, url.as_ref()).await
    }

    async fn error_for_status(
        response: Response<Body>,
        url: impl Into<String>,
    ) -> ClientResult<Bytes> {
        if response.status().is_client_error() || response.status().is_server_error() {
            Err(CustomClientError::Status {
                status: response.status(),
                url: url.into(),
            })
        } else {
            let bytes = hyper::body::to_bytes(response.into_body()).await?;

            Ok(bytes)
        }
    }

    pub async fn get_discord_attachment(&self, attachment: &Attachment) -> ClientResult<Bytes> {
        self.make_get_request(&attachment.url, Site::DiscordAttachment)
            .await
    }

    pub async fn get_osekai_medals(&self) -> ClientResult<Vec<OsekaiMedal>> {
        let url = "https://osekai.net/medals/api/medals.php";
        let form = &[("strSearch", "")];

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        let medals: OsekaiMedals = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiMedals))?;

        Ok(medals.0)
    }

    pub async fn get_osekai_beatmaps(&self, medal_name: &str) -> ClientResult<Vec<OsekaiMap>> {
        let url = "https://osekai.net/medals/api/beatmaps.php";
        let form = &[("strSearch", medal_name)];

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        let maps: OsekaiMaps = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiMaps))?;

        Ok(maps.0.unwrap_or_default())
    }

    pub async fn get_osekai_comments(&self, medal_name: &str) -> ClientResult<Vec<OsekaiComment>> {
        let url = "https://osekai.net/global/api/comment_system.php";
        let form = &[("strMedalName", medal_name), ("bGetComments", "true")];

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        let comments: OsekaiComments = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiComments))?;

        Ok(comments.0.unwrap_or_default())
    }

    pub async fn get_osekai_ranking<R: OsekaiRanking>(&self) -> ClientResult<Vec<R::Entry>> {
        let url = "https://osekai.net/rankings/api/api.php";
        let form = &[("App", R::FORM)];

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

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

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        let player: SnipePlayer = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipePlayer))?;

        Ok(player)
    }

    pub async fn get_snipe_country(&self, country: &str) -> ClientResult<Vec<SnipeCountryPlayer>> {
        let url = format!(
            "{HUISMETBENEN}rankings/{}/pp/weighted",
            country.to_lowercase()
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

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

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

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

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

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

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

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

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        let count: usize = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipeScoreCount))?;

        Ok(count)
    }

    pub async fn get_country_globals(
        &self,
        params: &OsuStatsListParams,
    ) -> ClientResult<Vec<OsuStatsPlayer>> {
        let mut map = Map::new();

        map.insert("rankMin".to_owned(), params.rank_min.into());
        map.insert("rankMax".to_owned(), params.rank_max.into());
        map.insert("gamemode".to_owned(), (params.mode as u8).into());
        map.insert("page".to_owned(), params.page.into());

        if let Some(ref country) = params.country {
            map.insert(COUNTRY.to_owned(), country.to_string().into());
        }

        let json = serde_json::to_vec(&map).map_err(CustomClientError::Serialize)?;
        let url = "https://osustats.ppy.sh/api/getScoreRanking";
        trace!("Requesting POST from url {url} [page {}]", params.page);

        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header(HYPER_USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, APPLICATION_JSON)
            .header(CONTENT_LENGTH, json.len())
            .body(Body::from(json))?;

        self.ratelimit(Site::OsuStats).await;

        let response = timeout(Duration::from_secs(4), self.client.request(req))
            .await
            .map_err(|_| CustomClientError::OsuStatsTimeout)??;

        let bytes = Self::error_for_status(response, url).await?;

        let players: Vec<OsuStatsPlayer> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::GlobalsList))?;

        Ok(players)
    }

    /// Be sure whitespaces in the username are **not** replaced
    pub async fn get_global_scores(
        &self,
        params: &OsuStatsParams,
    ) -> ClientResult<(Vec<OsuStatsScore>, usize)> {
        let mut map = Map::new();

        map.insert("accMin".to_owned(), params.acc_min.into());
        map.insert("accMax".to_owned(), params.acc_max.into());
        map.insert("rankMin".to_owned(), params.rank_min.into());
        map.insert("rankMax".to_owned(), params.rank_max.into());
        map.insert("gamemode".to_owned(), (params.mode as u8).into());
        map.insert("sortBy".to_owned(), (params.order as u8).into());
        map.insert("sortOrder".to_owned(), (!params.descending as u8).into());
        map.insert("page".to_owned(), params.page.into());
        map.insert("u1".to_owned(), params.username.to_string().into());

        if let Some(selection) = params.mods {
            let mod_str = match selection {
                ModSelection::Include(mods) => format!("+{mods}"),
                ModSelection::Exclude(mods) => format!("-{mods}"),
                ModSelection::Exact(mods) => format!("!{mods}"),
            };

            map.insert(MODS.to_owned(), mod_str.into());
        }

        let json = serde_json::to_vec(&map).map_err(CustomClientError::Serialize)?;
        let url = "https://osustats.ppy.sh/api/getScores";
        trace!("Requesting POST from url {url}");

        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header(HYPER_USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, APPLICATION_JSON)
            .header(CONTENT_LENGTH, json.len())
            .body(Body::from(json))?;

        self.ratelimit(Site::OsuStats).await;

        let response = timeout(Duration::from_secs(4), self.client.request(req))
            .await
            .map_err(|_| CustomClientError::OsuStatsTimeout)??;

        let bytes = Self::error_for_status(response, url).await?;

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

        let bytes = self.make_get_request(url, Site::OsuHiddenApi).await?;

        let scores: ScraperScores = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::Leaderboard))?;

        Ok(scores.get())
    }

    pub async fn get_avatar(&self, url: &str) -> ClientResult<Bytes> {
        self.make_get_request(url, Site::OsuAvatar).await
    }

    pub async fn get_badge(&self, url: &str) -> ClientResult<Bytes> {
        self.make_get_request(url, Site::OsuBadge).await
    }

    pub async fn get_mapset_cover(&self, covers: &BeatmapsetCovers) -> ClientResult<Bytes> {
        self.make_get_request(&covers.cover, Site::OsuMapsetCover)
            .await
    }

    pub async fn get_map_file(&self, map_id: u32) -> ClientResult<Bytes> {
        let url = format!("{OSU_BASE}osu/{map_id}");
        let backoff = ExponentialBackoff::new(2).factor(500).max_delay(10_000);
        const ATTEMPTS: usize = 10;

        for (duration, i) in backoff.take(ATTEMPTS).zip(1..) {
            let result = self.make_get_request(&url, Site::OsuMapFile).await;

            if matches!(&result, Err(CustomClientError::Status { status, ..}) if *status == StatusCode::TOO_MANY_REQUESTS)
                || matches!(&result, Ok(bytes) if bytes.starts_with(b"<html>"))
            {
                debug!("Request beatmap retry attempt #{i} | Backoff {duration:?}");
                sleep(duration).await;
            } else {
                return result;
            }
        }

        Err(CustomClientError::MapFileRetryLimit(map_id))
    }

    pub async fn get_rank_data(&self, mode: GameMode, param: RankParam) -> ClientResult<RankPP> {
        let key = &CONFIG.get().unwrap().tokens.osu_daily;
        let mut url = format!("{OSU_DAILY_API}pp.php?k={key}&m={}&", mode as u8);

        let _ = match param {
            RankParam::Rank(rank) => write!(url, "t=rank&v={rank}"),
            RankParam::Pp(pp) => write!(url, "t=pp&v={}", round(pp)),
        };

        let bytes = loop {
            match self.make_get_request(&url, Site::OsuDaily).await {
                Ok(bytes) => break bytes,
                Err(CustomClientError::Status { status, .. })
                    if status == StatusCode::TOO_MANY_REQUESTS =>
                {
                    debug!("Ratelimited by osudaily, wait a second");
                    sleep(Duration::from_secs(1)).await;
                }
                Err(err) => return Err(err),
            }
        };

        let rank_pp = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::RankData))?;

        Ok(rank_pp)
    }

    pub async fn get_twitch_user(&self, name: &str) -> ClientResult<Option<TwitchUser>> {
        let data = [("login", name)];

        let bytes = self
            .make_twitch_get_request(TWITCH_USERS_ENDPOINT, data)
            .await?;

        let mut users: TwitchDataList<TwitchUser> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::TwitchUserName))?;

        Ok(users.data.pop())
    }

    pub async fn get_twitch_user_by_id(&self, user_id: u64) -> ClientResult<Option<TwitchUser>> {
        let data = [("id", user_id)];

        let bytes = self
            .make_twitch_get_request(TWITCH_USERS_ENDPOINT, data)
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
                .make_twitch_get_request(TWITCH_USERS_ENDPOINT, data)
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
                .make_twitch_get_request(TWITCH_STREAM_ENDPOINT, data)
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
            .make_twitch_get_request(TWITCH_VIDEOS_ENDPOINT, data)
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
