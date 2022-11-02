use std::{fmt::Write, hash::Hash};

use bytes::Bytes;
use eyre::{Result, WrapErr};
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
use tokio::time::{sleep, timeout, Duration};
use twilight_model::channel::Attachment;

use crate::{
    commands::osu::OsuStatsPlayersArgs,
    core::BotConfig,
    util::{
        constants::{HUISMETBENEN, OSU_BASE},
        datetime::{DATE_FORMAT, TIME_FORMAT},
        hasher::IntHasher,
        osu::ModSelection,
        ExponentialBackoff,
    },
};

pub use self::{
    osekai::*, osu_stats::*, osu_tracker::*, respektive::*, rkyv_impls::UsernameWrapper, score::*,
    snipe::*,
};

#[cfg(feature = "twitch")]
pub use self::twitch::*;

use self::{rkyv_impls::*, score::ScraperScores};

mod deser;
mod osekai;
mod osu_stats;
mod osu_tracker;
mod respektive;
mod rkyv_impls;
mod score;
mod snipe;
mod twitch;

static MY_USER_AGENT: &str = env!("CARGO_PKG_NAME");

const APPLICATION_JSON: &str = "application/json";
const APPLICATION_URLENCODED: &str = "application/x-www-form-urlencoded";

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
#[repr(u8)]
enum Site {
    Cards,
    DiscordAttachment,
    Huismetbenen,
    Osekai,
    OsuAvatar,
    OsuBadge,
    OsuHiddenApi,
    OsuMapFile,
    OsuMapsetCover,
    OsuStats,
    OsuTracker,
    Respektive,
    #[cfg(feature = "twitch")]
    Twitch,
}

type Client = HyperClient<HttpsConnector<HttpConnector<GaiResolver>>, Body>;

pub struct CustomClient {
    client: Client,
    osu_session: &'static str,
    #[cfg(feature = "twitch")]
    twitch: TwitchData,
    ratelimiters: [LeakyBucket; 12 + cfg!(feature = "twitch") as usize],
}

impl CustomClient {
    pub async fn new() -> Result<Self> {
        let connector = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .build();

        let config = BotConfig::get();
        let client = HyperClient::builder().build(connector);

        #[cfg(feature = "twitch")]
        let twitch = {
            let twitch_client_id = &config.tokens.twitch_client_id;
            let twitch_token = &config.tokens.twitch_token;

            Self::get_twitch_token(&client, twitch_client_id, twitch_token)
                .await
                .wrap_err("failed to get twitch token")?
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
            ratelimiter(5),  // Cards
            ratelimiter(2),  // DiscordAttachment
            ratelimiter(2),  // Huismetbenen
            ratelimiter(2),  // Osekai
            ratelimiter(10), // OsuAvatar
            ratelimiter(10), // OsuBadge
            ratelimiter(2),  // OsuHiddenApi
            ratelimiter(5),  // OsuMapFile
            ratelimiter(10), // OsuMapsetCover
            ratelimiter(2),  // OsuStats
            ratelimiter(2),  // OsuTracker
            ratelimiter(1),  // Respektive
            #[cfg(feature = "twitch")]
            ratelimiter(5), // Twitch
        ];

        Ok(Self {
            client,
            osu_session: &config.tokens.osu_session,
            #[cfg(feature = "twitch")]
            twitch,
            ratelimiters,
        })
    }

    #[cfg(feature = "twitch")]
    async fn get_twitch_token(client: &Client, client_id: &str, token: &str) -> Result<TwitchData> {
        use crate::util::constants::TWITCH_OAUTH;

        let form = &[
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", token),
        ];

        let form_body = serde_urlencoded::to_string(form)?;
        let client_id = http::HeaderValue::from_str(client_id)?;

        let req = Request::builder()
            .method(Method::POST)
            .uri(TWITCH_OAUTH)
            .header("Client-ID", client_id.clone())
            .header(USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form_body))?;

        let response = client.request(req).await?;
        let bytes = Self::error_for_status(response, TWITCH_OAUTH).await?;

        let oauth_token = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize twitch token: {body}")
        })?;

        Ok(TwitchData {
            client_id,
            oauth_token,
        })
    }

    async fn ratelimit(&self, site: Site) {
        self.ratelimiters[site as usize].acquire_one().await
    }

    async fn make_get_request(&self, url: impl AsRef<str>, site: Site) -> Result<Bytes> {
        let url = url.as_ref();
        trace!("GET request of url {url}");

        let req = self.make_get_request_(url, site).body(Body::empty())?;

        self.ratelimit(site).await;

        let response = self
            .client
            .request(req)
            .await
            .wrap_err("failed to receive GET response")?;

        Self::error_for_status(response, url).await
    }

    #[cfg(feature = "twitch")]
    async fn make_twitch_get_request<I, U, V>(&self, url: impl AsRef<str>, data: I) -> Result<Bytes>
    where
        I: IntoIterator<Item = (U, V)>,
        U: std::fmt::Display,
        V: std::fmt::Display,
    {
        let url = url.as_ref();
        trace!("GET request of url {url}");

        let mut uri = format!("{url}?");
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

        let response = self
            .client
            .request(req)
            .await
            .wrap_err("failed to receive GET response from twitch")?;

        Self::error_for_status(response, url).await
    }

    fn make_get_request_(&self, url: impl AsRef<str>, site: Site) -> RequestBuilder {
        let req = Request::builder()
            .uri(url.as_ref())
            .method(Method::GET)
            .header(USER_AGENT, MY_USER_AGENT);

        match site {
            Site::OsuHiddenApi => req.header(COOKIE, format!("osu_session={}", self.osu_session)),
            #[cfg(feature = "twitch")]
            Site::Twitch => req
                .header("Client-ID", self.twitch.client_id.clone())
                .header(
                    http::header::AUTHORIZATION,
                    format!("Bearer {}", self.twitch.oauth_token),
                ),
            _ => req,
        }
    }

    async fn make_post_request<F>(
        &self,
        url: impl AsRef<str>,
        site: Site,
        form: &F,
    ) -> Result<Bytes>
    where
        F: Serialize,
    {
        let url = url.as_ref();
        trace!("POST request of url {url}");

        // TODO: use multipart
        let form_body = serde_urlencoded::to_string(form).wrap_err("failed to url encode")?;

        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header(USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, APPLICATION_URLENCODED)
            .body(Body::from(form_body))?;

        self.ratelimit(site).await;

        let response = self
            .client
            .request(req)
            .await
            .wrap_err("failed to receive POST response")?;

        Self::error_for_status(response, url).await
    }

    async fn error_for_status(response: Response<Body>, url: &str) -> Result<Bytes> {
        let status = response.status();

        ensure!(
            status.is_success(),
            "failed with status code {status} when requesting url {url}"
        );

        hyper::body::to_bytes(response.into_body())
            .await
            .wrap_err("failed to extract response bytes")
    }

    /// Turn the provided html into a .png image
    pub async fn html_to_png(&self, html: &str) -> Result<Bytes> {
        let url = "http://localhost:7227";
        let form = &[("html", html)];

        self.make_post_request(url, Site::Cards, form).await
    }

    pub async fn get_discord_attachment(&self, attachment: &Attachment) -> Result<Bytes> {
        self.make_get_request(&attachment.url, Site::DiscordAttachment)
            .await
    }

    pub async fn get_respektive_osustats_counts(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Result<Option<RespektiveTopCount>> {
        let mode = mode as u8;
        let url = format!("https://osustats.respektive.pw/counts/{user_id}?mode={mode}");

        // Manual request so the potential 404 error is not wrapped in a Report
        let req = self
            .make_get_request_(&url, Site::Respektive)
            .body(Body::empty())?;

        self.ratelimit(Site::Respektive).await;

        let response = self
            .client
            .request(req)
            .await
            .wrap_err("failed to receive GET response")?;

        let status = response.status();

        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        ensure!(
            status.is_success(),
            "failed with status code {status} when requesting url {url}"
        );

        let bytes = hyper::body::to_bytes(response.into_body())
            .await
            .wrap_err("failed to extract response bytes")?;

        serde_json::from_slice(&bytes)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("failed to deserialize respektive top count: {body}")
            })
            .map(Some)
    }

    pub async fn get_respektive_user(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Result<Option<RespektiveUser>> {
        let url = format!("https://score.respektive.pw/u/{user_id}?m={}", mode as u8);
        let bytes = self.make_get_request(url, Site::Respektive).await?;

        let mut users: Vec<RespektiveUser> =
            serde_json::from_slice(&bytes).wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("failed to deserialize respektive user: {body}")
            })?;

        Ok(users.pop().filter(|user| user.rank > 0))
    }

    pub async fn get_respektive_rank(
        &self,
        rank: u32,
        mode: GameMode,
    ) -> Result<Option<RespektiveUser>> {
        let url = format!("https://score.respektive.pw/rank/{rank}?m={}", mode as u8);
        let bytes = self.make_get_request(url, Site::Respektive).await?;

        let mut users: Vec<RespektiveUser> =
            serde_json::from_slice(&bytes).wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("failed to deserialize respektive rank: {body}")
            })?;

        Ok(users.pop().filter(|user| user.rank > 0))
    }

    pub async fn get_osutracker_country_details(
        &self,
        country_code: Option<&str>,
    ) -> Result<OsuTrackerCountryDetails> {
        let url = format!(
            "https://osutracker.com/api/countries/{code}/details",
            code = country_code.unwrap_or("Global"),
        );

        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker country details: {body}")
        })
    }

    /// Don't use this; use [`RedisCache::osutracker_stats`](crate::core::RedisCache::osutracker_stats) instead.
    pub async fn get_osutracker_stats(&self) -> Result<OsuTrackerStats> {
        let url = "https://osutracker.com/api/stats";
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker stats: {body}")
        })
    }

    /// Don't use this; use [`RedisCache::osutracker_pp_group`](crate::core::RedisCache::osutracker_pp_group) instead.
    pub async fn get_osutracker_pp_group(&self, pp: u32) -> Result<OsuTrackerPpGroup> {
        let url = format!("https://osutracker.com/api/stats/ppBarrier?number={pp}");
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker pp groups: {body}")
        })
    }

    /// Don't use this; use [`RedisCache::osutracker_counts`](crate::core::RedisCache::osutracker_counts) instead.
    pub async fn get_osutracker_counts(&self) -> Result<Vec<OsuTrackerIdCount>> {
        let url = "https://osutracker.com/api/stats/idCounts";
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker id counts: {body}")
        })
    }

    /// Don't use this; use [`RedisCache::badges`](crate::core::RedisCache::badges) instead.
    pub async fn get_osekai_badges(&self) -> Result<Vec<OsekaiBadge>> {
        let url = "https://osekai.net/badges/api/getBadges.php";

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai badges: {body}")
        })
    }

    pub async fn get_osekai_badge_owners(&self, badge_id: u32) -> Result<Vec<OsekaiBadgeOwner>> {
        let url = format!("https://osekai.net/badges/api/getUsers.php?badge_id={badge_id}");
        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai badge owners: {body}")
        })
    }

    /// Don't use this; use [`RedisCache::medals`](crate::core::RedisCache::medals) instead.
    pub async fn get_osekai_medals(&self) -> Result<Vec<OsekaiMedal>> {
        let url = "https://osekai.net/medals/api/medals.php";
        let form = &[("strSearch", "")];
        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        let medals: OsekaiMedals = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai medals: {body}")
        })?;

        Ok(medals.0)
    }

    pub async fn get_osekai_beatmaps(&self, medal_name: &str) -> Result<Vec<OsekaiMap>> {
        let url = "https://osekai.net/medals/api/beatmaps.php";
        let form = &[("strSearch", medal_name)];

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        let maps: OsekaiMaps = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai maps: {body}")
        })?;

        Ok(maps.0.unwrap_or_default())
    }

    pub async fn get_osekai_comments(&self, medal_name: &str) -> Result<Vec<OsekaiComment>> {
        let url = "https://osekai.net/global/api/comment_system.php";
        let form = &[("strMedalName", medal_name), ("bGetComments", "true")];

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        let comments: OsekaiComments = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai comments: {body}")
        })?;

        Ok(comments.0.unwrap_or_default())
    }

    /// Don't use this; use [`RedisCache::osekai_ranking`](crate::core::RedisCache::osekai_ranking) instead.
    pub async fn get_osekai_ranking<R: OsekaiRanking>(&self) -> Result<Vec<R::Entry>> {
        let url = "https://osekai.net/rankings/api/api.php";
        let form = &[("App", R::FORM)];

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        serde_json::from_slice::<OsekaiRankingEntries<R>>(&bytes)
            .map(Vec::from)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("failed to deserialize Osekai {}: {body}", R::FORM)
            })
    }

    pub async fn get_snipe_player(&self, country: &str, user_id: u32) -> Result<SnipePlayer> {
        let url = format!(
            "{HUISMETBENEN}player/{}/{user_id}?type=id",
            country.to_lowercase(),
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe player: {body}")
        })
    }

    pub async fn get_snipe_country(&self, country: &str) -> Result<Vec<SnipeCountryPlayer>> {
        let url = format!(
            "{HUISMETBENEN}rankings/{}/pp/weighted",
            country.to_lowercase()
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe country: {body}")
        })
    }

    pub async fn get_country_statistics(&self, country: &str) -> Result<SnipeCountryStatistics> {
        let country = country.to_lowercase();
        let url = format!("{HUISMETBENEN}rankings/{country}/statistics");

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize country statistics: {body}")
        })
    }

    pub async fn get_national_snipes(
        &self,
        user: &User,
        sniper: bool,
        from: OffsetDateTime,
        until: OffsetDateTime,
    ) -> Result<Vec<SnipeRecent>> {
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

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe recent: {body}")
        })
    }

    pub async fn get_national_firsts(&self, params: &SnipeScoreParams) -> Result<Vec<SnipeScore>> {
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

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe score: {body}")
        })
    }

    pub async fn get_national_firsts_count(&self, params: &SnipeScoreParams) -> Result<usize> {
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

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe score count: {body}")
        })
    }

    pub async fn get_country_globals(
        &self,
        params: &OsuStatsPlayersArgs,
    ) -> Result<Vec<OsuStatsPlayer>> {
        let mut map = Map::new();

        map.insert("rankMin".to_owned(), params.min_rank.into());
        map.insert("rankMax".to_owned(), params.max_rank.into());
        map.insert("gamemode".to_owned(), (params.mode as u8).into());
        map.insert("page".to_owned(), params.page.into());

        if let Some(ref country) = params.country {
            map.insert("country".to_owned(), country.to_string().into());
        }

        let json = serde_json::to_vec(&map).wrap_err("failed to serialize")?;
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
            .map_err(|_| eyre!("timeout while waiting for osustats"))??;

        let bytes = Self::error_for_status(response, url).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize globals list: {body}")
        })
    }

    /// Be sure whitespaces in the username are **not** replaced
    pub async fn get_global_scores(
        &self,
        params: &OsuStatsParams,
    ) -> Result<(Vec<OsuStatsScore>, usize)> {
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

        let json = serde_json::to_vec(&map).wrap_err("failed to serialize")?;
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
            .map_err(|_| eyre!("timeout while waiting for osustats"))??;

        let status = response.status();

        // Don't use Self::error_for_status since osustats returns a 400
        // if the user has no scores for the given parameters
        let bytes = if (status.is_client_error() && status != StatusCode::BAD_REQUEST)
            || status.is_server_error()
        {
            bail!("failed with status code {status} when requesting url {url}")
        } else {
            hyper::body::to_bytes(response.into_body())
                .await
                .wrap_err("failed to extract response bytes")?
        };

        let result: Value = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osustats global: {body}")
        })?;

        let (scores, amount) = if let Value::Array(mut array) = result {
            let mut values = array.drain(..2);

            let scores = serde_json::from_value(values.next().unwrap()).wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("failed to deserialize osustats global scores: {body}")
            })?;

            let amount = serde_json::from_value(values.next().unwrap()).wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("failed to deserialize osustats global amount: {body}")
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
    ) -> Result<Vec<ScraperScore>> {
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
            let mut uniques = HashSet::with_capacity_and_hasher(50, IntHasher);
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
            let mut uniques = HashSet::with_capacity_and_hasher(50, IntHasher);
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
    ) -> Result<Vec<ScraperScore>> {
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

        let scores: ScraperScores = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize leaderboard: {body}")
        })?;

        Ok(scores.get())
    }

    pub async fn get_avatar(&self, url: &str) -> Result<Bytes> {
        self.make_get_request(url, Site::OsuAvatar).await
    }

    pub async fn get_badge(&self, url: &str) -> Result<Bytes> {
        self.make_get_request(url, Site::OsuBadge).await
    }

    /// Make sure you provide a valid url to a mapset cover
    pub async fn get_mapset_cover(&self, cover: &str) -> Result<Bytes> {
        self.make_get_request(&cover, Site::OsuMapsetCover).await
    }

    pub async fn get_map_file(&self, map_id: u32) -> Result<Bytes> {
        let url = format!("{OSU_BASE}osu/{map_id}");
        let backoff = ExponentialBackoff::new(2).factor(500).max_delay(10_000);
        const ATTEMPTS: usize = 10;

        for (duration, i) in backoff.take(ATTEMPTS).zip(1..) {
            let result = self.make_get_request(&url, Site::OsuMapFile).await;

            if matches!(&result, Ok(bytes) if bytes.starts_with(b"<html>")) {
                debug!("Request beatmap retry attempt #{i} | Backoff {duration:?}");
                sleep(duration).await;
            } else {
                return result;
            }
        }

        bail!("reached retry limit and still failed to download {map_id}.osu")
    }
}

#[cfg(feature = "twitch")]
mod twitch_impls {
    use std::{borrow::Cow, time::Duration};

    use eyre::{Result, WrapErr};
    use tokio::time::interval;

    use crate::util::constants::{
        TWITCH_STREAM_ENDPOINT, TWITCH_USERS_ENDPOINT, TWITCH_VIDEOS_ENDPOINT,
    };

    use super::{CustomClient, TwitchDataList, TwitchStream, TwitchUser, TwitchVideo};

    impl CustomClient {
        pub async fn get_twitch_user(&self, name: &str) -> Result<Option<TwitchUser>> {
            let data = [("login", name)];

            let bytes = self
                .make_twitch_get_request(TWITCH_USERS_ENDPOINT, data)
                .await?;

            let mut users: TwitchDataList<TwitchUser> = serde_json::from_slice(&bytes)
                .wrap_err_with(|| {
                    let body = String::from_utf8_lossy(&bytes);

                    format!("failed to deserialize twitch username: {body}")
                })?;

            Ok(users.data.pop())
        }

        pub async fn get_twitch_user_by_id(&self, user_id: u64) -> Result<Option<TwitchUser>> {
            let data = [("id", user_id)];

            let bytes = self
                .make_twitch_get_request(TWITCH_USERS_ENDPOINT, data)
                .await?;

            let mut users: TwitchDataList<TwitchUser> = serde_json::from_slice(&bytes)
                .wrap_err_with(|| {
                    let body = String::from_utf8_lossy(&bytes);

                    format!("failed to deserialize twitch user id: {body}")
                })?;

            Ok(users.data.pop())
        }

        pub async fn get_twitch_users(&self, user_ids: &[u64]) -> Result<Vec<TwitchUser>> {
            let mut users = Vec::with_capacity(user_ids.len());

            for chunk in user_ids.chunks(100) {
                let data: Vec<_> = chunk.iter().map(|&id| ("id", id)).collect();

                let bytes = self
                    .make_twitch_get_request(TWITCH_USERS_ENDPOINT, data)
                    .await?;

                let parsed_response: TwitchDataList<TwitchUser> = serde_json::from_slice(&bytes)
                    .wrap_err_with(|| {
                        let body = String::from_utf8_lossy(&bytes);

                        format!("failed to deserialize twitch users: {body}")
                    })?;

                users.extend(parsed_response.data);
            }

            Ok(users)
        }

        pub async fn get_twitch_streams(&self, user_ids: &[u64]) -> Result<Vec<TwitchStream>> {
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
                    .wrap_err_with(|| {
                        let body = String::from_utf8_lossy(&bytes);

                        format!("failed to deserialize twitch streams: {body}")
                    })?;

                streams.extend(parsed_response.data);
            }

            Ok(streams)
        }

        pub async fn get_last_twitch_vod(&self, user_id: u64) -> Result<Option<TwitchVideo>> {
            let data = [
                ("user_id", Cow::Owned(user_id.to_string())),
                ("first", "1".into()),
                ("sort", "time".into()),
            ];

            let bytes = self
                .make_twitch_get_request(TWITCH_VIDEOS_ENDPOINT, data)
                .await?;

            let mut videos: TwitchDataList<TwitchVideo> = serde_json::from_slice(&bytes)
                .wrap_err_with(|| {
                    let body = String::from_utf8_lossy(&bytes);

                    format!("failed to deserialize twitch videos: {body}")
                })?;

            Ok(videos.data.pop())
        }
    }
}
