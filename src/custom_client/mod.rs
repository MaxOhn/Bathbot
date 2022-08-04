use std::{
    borrow::Cow,
    fmt::{Display, Write},
    hash::Hash,
};

use bytes::Bytes;
use hashbrown::HashSet;
use http::{
    header::{CONTENT_LENGTH, COOKIE},
    request::Builder as RequestBuilder,
    Response, StatusCode,
};
use hyper::{
    client::{connect::dns::GaiResolver, Client as HyperClient, HttpConnector},
    header::{CONTENT_TYPE, USER_AGENT},
    Body, Method, Request,
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use leaky_bucket_lite::LeakyBucket;
use rosu_v2::prelude::{GameMode, GameMods, User};
use serde::Serialize;
use serde_json::{Map, Value};
use time::{format_description::FormatItem, OffsetDateTime};
use tokio::time::{interval, sleep, timeout, Duration};
use twilight_model::channel::Attachment;

use crate::{
    commands::osu::OsuStatsPlayersArgs,
    core::BotConfig,
    util::{
        constants::{
            HUISMETBENEN, OSU_BASE, OSU_DAILY_API, TWITCH_STREAM_ENDPOINT, TWITCH_USERS_ENDPOINT,
            TWITCH_VIDEOS_ENDPOINT,
        },
        datetime::{DATE_FORMAT, TIME_FORMAT},
        hasher::SimpleBuildHasher,
        numbers::round,
        osu::ModSelection,
        ExponentialBackoff,
    },
    CONFIG,
};

#[cfg(not(debug_assertions))]
use http::header::AUTHORIZATION;

#[cfg(not(debug_assertions))]
use hyper::header::HeaderValue;

#[cfg(not(debug_assertions))]
use crate::util::constants::TWITCH_OAUTH;

pub use self::{
    error::*, osekai::*, osu_daily::*, osu_stats::*, osu_tracker::*, respektive::*,
    rkyv_impls::UsernameWrapper, score::*, snipe::*, twitch::*,
};

use self::{rkyv_impls::*, score::ScraperScores};

mod deserialize;
mod error;
mod osekai;
mod osu_daily;
mod osu_stats;
mod osu_tracker;
mod respektive;
mod rkyv_impls;
mod score;
mod snipe;
mod twitch;

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
    OsuTracker,
    Respektive,
    #[cfg(not(debug_assertions))]
    Twitch,
}

type Client = HyperClient<HttpsConnector<HttpConnector<GaiResolver>>, Body>;

pub struct CustomClient {
    client: Client,
    osu_session: &'static str,
    #[cfg(not(debug_assertions))]
    twitch: TwitchData,
    ratelimiters: [LeakyBucket; 12 + !cfg!(debug_assertions) as usize],
}

#[cfg(not(debug_assertions))]
struct TwitchData {
    client_id: HeaderValue,
    oauth_token: TwitchOAuthToken,
}

impl CustomClient {
    pub async fn new(config: &'static BotConfig) -> ClientResult<Self> {
        let connector = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .build();

        let client = HyperClient::builder().build(connector);

        #[cfg(not(debug_assertions))]
        let twitch = {
            let twitch_client_id = &config.tokens.twitch_client_id;
            let twitch_token = &config.tokens.twitch_token;

            Self::get_twitch_token(&client, twitch_client_id, twitch_token).await?
        };

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
            ratelimiter(2),  // OsuTracker
            ratelimiter(1),  // Respektive
            #[cfg(not(debug_assertions))]
            ratelimiter(5), // Twitch
        ];

        Ok(Self {
            client,
            osu_session: &config.tokens.osu_session,
            #[cfg(not(debug_assertions))]
            twitch,
            ratelimiters,
        })
    }

    #[cfg(not(debug_assertions))]
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
            .header(USER_AGENT, MY_USER_AGENT)
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

    #[cfg(debug_assertions)]
    async fn make_twitch_get_request<I, U, V>(
        &self,
        _: impl AsRef<str>,
        _: I,
    ) -> ClientResult<Bytes>
    where
        I: IntoIterator<Item = (U, V)>,
        U: Display,
        V: Display,
    {
        Err(CustomClientError::NoTwitchOnDebug)
    }

    #[cfg(not(debug_assertions))]
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
            .header(USER_AGENT, MY_USER_AGENT);

        match site {
            Site::OsuHiddenApi => req.header(COOKIE, format!("osu_session={}", self.osu_session)),
            #[cfg(not(debug_assertions))]
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
            .header(USER_AGENT, MY_USER_AGENT)
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

    pub async fn get_respektive_user(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> ClientResult<Option<RespektiveUser>> {
        let url = format!("https://score.respektive.pw/u/{user_id}?m={}", mode as u8);
        let bytes = self.make_get_request(url, Site::Respektive).await?;

        let mut users: Vec<RespektiveUser> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::RespektiveUser))?;

        Ok(users.pop().filter(|user| user.rank > 0))
    }

    pub async fn get_respektive_rank(
        &self,
        rank: u32,
        mode: GameMode,
    ) -> ClientResult<Option<RespektiveUser>> {
        let url = format!("https://score.respektive.pw/rank/{rank}?m={}", mode as u8);
        let bytes = self.make_get_request(url, Site::Respektive).await?;

        let mut users: Vec<RespektiveUser> = serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::RespektiveRank))?;

        Ok(users.pop().filter(|user| user.rank > 0))
    }

    pub async fn get_osutracker_country_details(
        &self,
        country_code: &str,
    ) -> ClientResult<OsuTrackerCountryDetails> {
        let url = format!("https://osutracker.com/api/countries/{country_code}/details");
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsuTrackerCountryDetails))
    }

    /// Don't use this; use [`RedisCache::osutracker_stats`](crate::core::RedisCache::osutracker_stats) instead.
    pub async fn get_osutracker_stats(&self) -> ClientResult<OsuTrackerStats> {
        let url = "https://osutracker.com/api/stats";
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsuTrackerStats))
    }

    /// Don't use this; use [`RedisCache::osutracker_pp_group`](crate::core::RedisCache::osutracker_pp_group) instead.
    pub async fn get_osutracker_pp_group(&self, pp: u32) -> ClientResult<OsuTrackerPpGroup> {
        let url = format!("https://osutracker.com/api/stats/ppBarrier?number={pp}");
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsuTrackerPpGroup))
    }

    /// Don't use this; use [`RedisCache::osutracker_counts`](crate::core::RedisCache::osutracker_counts) instead.
    pub async fn get_osutracker_counts(&self) -> ClientResult<Vec<OsuTrackerIdCount>> {
        let url = "https://osutracker.com/api/stats/idCounts";
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsuTrackerIdCounts))
    }

    /// Don't use this; use [`RedisCache::badges`](crate::core::RedisCache::badges) instead.
    pub async fn get_osekai_badges(&self) -> ClientResult<Vec<OsekaiBadge>> {
        let url = "https://osekai.net/badges/api/getBadges.php";

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiBadges))
    }

    pub async fn get_osekai_badge_owners(
        &self,
        badge_id: u32,
    ) -> ClientResult<Vec<OsekaiBadgeOwner>> {
        let url = format!("https://osekai.net/badges/api/getUsers.php?badge_id={badge_id}");

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiBadgeOwners))
    }

    /// Don't use this; use [`RedisCache::medals`](crate::core::RedisCache::medals) instead.
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

    /// Don't use this; use [`RedisCache::osekai_ranking`](crate::core::RedisCache::osekai_ranking) instead.
    pub async fn get_osekai_ranking<R: OsekaiRanking>(&self) -> ClientResult<Vec<R::Entry>> {
        let url = "https://osekai.net/rankings/api/api.php";
        let form = &[("App", R::FORM)];

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        serde_json::from_slice(&bytes).map_err(|e| {
            CustomClientError::parsing(e, &bytes, ErrorKind::OsekaiRanking(R::REQUEST))
        })
    }

    pub async fn get_snipe_player(&self, country: &str, user_id: u32) -> ClientResult<SnipePlayer> {
        let url = format!(
            "{HUISMETBENEN}player/{}/{user_id}?type=id",
            country.to_lowercase(),
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipePlayer))
    }

    pub async fn get_snipe_country(&self, country: &str) -> ClientResult<Vec<SnipeCountryPlayer>> {
        let url = format!(
            "{HUISMETBENEN}rankings/{}/pp/weighted",
            country.to_lowercase()
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipeCountry))
    }

    pub async fn get_country_statistics(
        &self,
        country: &str,
    ) -> ClientResult<SnipeCountryStatistics> {
        let country = country.to_lowercase();
        let url = format!("{HUISMETBENEN}rankings/{country}/statistics");

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::CountryStatistics))
    }

    pub async fn get_national_snipes(
        &self,
        user: &User,
        sniper: bool,
        from: OffsetDateTime,
        until: OffsetDateTime,
    ) -> ClientResult<Vec<SnipeRecent>> {
        pub const DATETIME_FORMAT: &[FormatItem<'_>] = &[
            FormatItem::Compound(DATE_FORMAT),
            FormatItem::Literal(b"T"),
            FormatItem::Compound(TIME_FORMAT),
            FormatItem::Literal(b"Z"),
        ];

        let url = format!(
            "{HUISMETBENEN}snipes/{}/{}?since={}&until={}",
            user.user_id,
            if sniper { "new" } else { "old" },
            from.format(DATETIME_FORMAT).unwrap(),
            until.format(DATETIME_FORMAT).unwrap()
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipeRecent))
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

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipeScore))
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

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::SnipeScoreCount))
    }

    pub async fn get_country_globals(
        &self,
        params: &OsuStatsPlayersArgs,
    ) -> ClientResult<Vec<OsuStatsPlayer>> {
        let mut map = Map::new();

        map.insert("rankMin".to_owned(), params.min_rank.into());
        map.insert("rankMax".to_owned(), params.max_rank.into());
        map.insert("gamemode".to_owned(), (params.mode as u8).into());
        map.insert("page".to_owned(), params.page.into());

        if let Some(ref country) = params.country {
            map.insert("country".to_owned(), country.to_string().into());
        }

        let json = serde_json::to_vec(&map).map_err(CustomClientError::Serialize)?;
        let url = "https://osustats.ppy.sh/api/getScoreRanking";
        trace!("Requesting POST from url {url} [page {}]", params.page);

        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header(USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, APPLICATION_JSON)
            .header(CONTENT_LENGTH, json.len())
            .body(Body::from(json))?;

        self.ratelimit(Site::OsuStats).await;

        let response = timeout(Duration::from_secs(4), self.client.request(req))
            .await
            .map_err(|_| CustomClientError::OsuStatsTimeout)??;

        let bytes = Self::error_for_status(response, url).await?;

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::GlobalsList))
    }

    /// Be sure whitespaces in the username are **not** replaced
    pub async fn get_global_scores(
        &self,
        params: &OsuStatsParams,
    ) -> ClientResult<(Vec<OsuStatsScore>, usize)> {
        let mut map = Map::new();

        map.insert("accMin".to_owned(), params.min_acc.into());
        map.insert("accMax".to_owned(), params.max_acc.into());
        map.insert("rankMin".to_owned(), params.min_rank.into());
        map.insert("rankMax".to_owned(), params.max_rank.into());
        map.insert("gamemode".to_owned(), (params.mode as u8).into());
        map.insert("sortBy".to_owned(), (params.order as u8).into());
        map.insert(
            "sortOrder".to_owned(),
            (!params.descending as u8).to_string().into(), // required as string
        );
        map.insert("page".to_owned(), params.page.into());
        map.insert("u1".to_owned(), params.username.to_string().into());

        if let Some(selection) = params.mods {
            let mod_str = match selection {
                ModSelection::Include(mods) => format!("+{mods}"),
                ModSelection::Exclude(mods) => format!("-{mods}"),
                ModSelection::Exact(mods) => format!("!{mods}"),
            };

            map.insert("mods".to_owned(), mod_str.into());
        }

        let json = serde_json::to_vec(&map).map_err(CustomClientError::Serialize)?;
        let url = "https://osustats.ppy.sh/api/getScores";
        trace!("Requesting POST from url {url}");

        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header(USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, APPLICATION_JSON)
            .header(CONTENT_LENGTH, json.len())
            .body(Body::from(json))?;

        self.ratelimit(Site::OsuStats).await;

        let response = timeout(Duration::from_secs(4), self.client.request(req))
            .await
            .map_err(|_| CustomClientError::OsuStatsTimeout)??;

        let status = response.status();

        // Don't use Self::error_for_status since osustats returns a 400
        // if the user has no scores for the given parameters
        let bytes = if (status.is_client_error() && status != StatusCode::BAD_REQUEST)
            || status.is_server_error()
        {
            return Err(CustomClientError::Status {
                status,
                url: url.to_owned(),
            });
        } else {
            hyper::body::to_bytes(response.into_body()).await?
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

    // Retrieve the global leaderboard of a map
    // If mods contain DT / NC, it will do another request for the opposite
    // If mods dont contain Mirror and its a mania map, it will perform the
    // same requests again but with Mirror enabled
    pub async fn get_leaderboard(
        &self,
        map_id: u32,
        mods: Option<GameMods>,
        mode: GameMode,
    ) -> ClientResult<Vec<ScraperScore>> {
        let mut scores = self._get_leaderboard(map_id, mods).await?;

        let non_mirror = mods
            .map(|mods| !mods.contains(GameMods::Mirror))
            .unwrap_or(true);

        // Check if another request for mania's MR is needed
        if mode == GameMode::Mania && non_mirror {
            let mods = match mods {
                None => Some(GameMods::Mirror),
                Some(mods) => Some(mods | GameMods::Mirror),
            };

            let mut new_scores = self._get_leaderboard(map_id, mods).await?;
            scores.append(&mut new_scores);
            scores.sort_unstable_by(|a, b| b.score.cmp(&a.score));
            let mut uniques = HashSet::with_capacity_and_hasher(50, SimpleBuildHasher);
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
            if mode == GameMode::Mania && non_mirror {
                let mods = mods.map(|mods| mods | GameMods::Mirror);
                let mut new_scores = self._get_leaderboard(map_id, mods).await?;
                scores.append(&mut new_scores);
            }

            let mut new_scores = self._get_leaderboard(map_id, mods).await?;
            scores.append(&mut new_scores);
            scores.sort_unstable_by(|a, b| b.score.cmp(&a.score));
            let mut uniques = HashSet::with_capacity_and_hasher(50, SimpleBuildHasher);
            scores.retain(|s| uniques.insert(s.user_id));
            scores.truncate(50);
        }

        Ok(scores)
    }

    // Retrieve the global leaderboard of a map
    async fn _get_leaderboard(
        &self,
        map_id: u32,
        mods: Option<GameMods>,
    ) -> ClientResult<Vec<ScraperScore>> {
        let mut url = format!("{OSU_BASE}beatmaps/{map_id}/scores?");

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

    /// Make sure you provide a valid url to a mapset cover
    pub async fn get_mapset_cover(&self, cover: &str) -> ClientResult<Bytes> {
        self.make_get_request(&cover, Site::OsuMapsetCover).await
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

        serde_json::from_slice(&bytes)
            .map_err(|e| CustomClientError::parsing(e, &bytes, ErrorKind::RankData))
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
            let mut data: Vec<_> = chunk.iter().map(|&id| ("user_id", id)).collect();
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
            ("user_id", Cow::Owned(user_id.to_string())),
            ("first", "1".into()),
            ("sort", "time".into()),
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
