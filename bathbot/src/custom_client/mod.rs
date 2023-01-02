use std::{fmt::Write, hash::Hash};

use bytes::Bytes;
use eyre::{Report, Result, WrapErr};
use hashbrown::HashSet;
use http::{
    header::{CONTENT_LENGTH, COOKIE},
    request::Builder as RequestBuilder,
    Response,
};
use hyper::{
    client::{connect::dns::GaiResolver, Client as HyperClient, HttpConnector},
    header::{CONTENT_TYPE, USER_AGENT},
    Body, Method, Request,
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use leaky_bucket_lite::LeakyBucket;
use rosu_v2::prelude::{GameMode, GameMods};
use serde_json::Value;
use thiserror::Error;
use time::{format_description::FormatItem, OffsetDateTime};
use tokio::time::{timeout, Duration};
use twilight_model::channel::Attachment;

use crate::{
    commands::osu::OsuStatsPlayersArgs,
    core::BotConfig,
    util::{
        constants::{HUISMETBENEN, OSU_BASE},
        datetime::{DATE_FORMAT, TIME_FORMAT},
        hasher::IntHasher,
        osu::ModSelection,
    },
};

pub use self::{
    osekai::*, osu_stats::*, osu_tracker::*, respektive::*, rkyv_impls::UsernameWrapper, score::*,
    snipe::*,
};

#[cfg(feature = "twitch")]
pub use self::twitch::*;

use self::{multipart::Multipart, rkyv_impls::*, score::ScraperScores};

mod deser;
mod multipart;
mod osekai;
mod osu_stats;
mod osu_tracker;
mod respektive;
mod rkyv_impls;
mod score;
mod snipe;
mod twitch;

static MY_USER_AGENT: &str = env!("CARGO_PKG_NAME");

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
#[repr(u8)]
enum Site {
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
    ratelimiters: [LeakyBucket; 11 + cfg!(feature = "twitch") as usize],
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
            ratelimiter(2),  // DiscordAttachment
            ratelimiter(2),  // Huismetbenen
            ratelimiter(2),  // Osekai
            ratelimiter(10), // OsuAvatar
            ratelimiter(10), // OsuBadge
            ratelimiter(2),  // OsuHiddenApi
            ratelimiter(3),  // OsuMapFile
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

        let form = Multipart::new()
            .push_text("grant_type", "client_credentials")
            .push_text("client_id", client_id)
            .push_text("client_secret", token);

        let client_id = http::HeaderValue::from_str(client_id)?;
        let content_type = format!("multipart/form-data; boundary={}", form.boundary());
        let form = form.finish();

        let req = Request::builder()
            .method(Method::POST)
            .uri(TWITCH_OAUTH)
            .header(USER_AGENT, MY_USER_AGENT)
            .header("Client-ID", client_id.clone())
            .header(CONTENT_TYPE, content_type)
            .header(CONTENT_LENGTH, form.len())
            .body(Body::from(form))
            .wrap_err("failed to build POST request")?;

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

    async fn make_get_request(
        &self,
        url: impl AsRef<str>,
        site: Site,
    ) -> Result<Bytes, ClientError> {
        let url = url.as_ref();
        trace!("GET request of url {url}");

        let req = self
            .make_get_request_(url, site)
            .body(Body::empty())
            .wrap_err("failed to build GET request")?;

        self.ratelimit(site).await;

        let response = self
            .client
            .request(req)
            .await
            .wrap_err("failed to receive GET response")?;

        Self::error_for_status(response, url).await
    }

    #[cfg(feature = "twitch")]
    async fn make_twitch_get_request<I, U, V>(
        &self,
        url: impl AsRef<str>,
        data: I,
    ) -> Result<Bytes, ClientError>
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
            .body(Body::empty())
            .wrap_err("failed to build twitch GET request")?;

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

    async fn make_post_request(
        &self,
        url: impl AsRef<str>,
        site: Site,
        form: Multipart,
    ) -> Result<Bytes, ClientError> {
        let url = url.as_ref();
        trace!("POST request of url {url}");

        let content_type = format!("multipart/form-data; boundary={}", form.boundary());
        let form = form.finish();

        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header(USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, content_type)
            .header(CONTENT_LENGTH, form.len())
            .body(Body::from(form))
            .wrap_err("failed to build POST request")?;

        self.ratelimit(site).await;

        let response = self
            .client
            .request(req)
            .await
            .wrap_err("failed to receive POST response")?;

        Self::error_for_status(response, url).await
    }

    async fn error_for_status(response: Response<Body>, url: &str) -> Result<Bytes, ClientError> {
        let status = response.status();

        match status.as_u16() {
            _code @ 200..=299 => hyper::body::to_bytes(response.into_body())
                .await
                .wrap_err("failed to extract response bytes")
                .map_err(ClientError::Report),
            400 => Err(ClientError::BadRequest),
            404 => Err(ClientError::NotFound),
            429 => Err(ClientError::Ratelimited),
            _ => Err(eyre!("failed with status code {status} when requesting url {url}").into()),
        }
    }

    pub async fn check_skin_url(&self, url: &str) -> Result<Response<Body>, ClientError> {
        trace!("HEAD request of url {url}");

        let req = Request::builder()
            .uri(url)
            .method(Method::HEAD)
            .header(USER_AGENT, MY_USER_AGENT)
            .body(Body::empty())
            .wrap_err("failed to build HEAD request")?;

        let response = self
            .client
            .request(req)
            .await
            .wrap_err("failed to receive HEAD response")?;

        let status = response.status();

        if (200..=299).contains(&status.as_u16()) {
            Ok(response)
        } else {
            Err(eyre!("failed with status code {status} when requesting url {url}").into())
        }
    }

    pub async fn get_discord_attachment(&self, attachment: &Attachment) -> Result<Bytes> {
        self.make_get_request(&attachment.url, Site::DiscordAttachment)
            .await
            .map_err(Report::new)
    }

    pub async fn get_respektive_osustats_counts(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Result<Option<RespektiveTopCount>> {
        let mode = mode as u8;
        let url = format!("https://osustats.respektive.pw/counts/{user_id}?mode={mode}");

        let bytes = match self.make_get_request(url, Site::Respektive).await {
            Ok(bytes) => bytes,
            Err(ClientError::NotFound) => return Ok(None),
            Err(err) => return Err(Report::new(err)),
        };

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
        let form = Multipart::new().push_text("strSearch", "");

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        let medals: OsekaiMedals = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai medals: {body}")
        })?;

        Ok(medals.0)
    }

    pub async fn get_osekai_beatmaps(&self, medal_name: &str) -> Result<Vec<OsekaiMap>> {
        let url = "https://osekai.net/medals/api/beatmaps.php";
        let form = Multipart::new().push_text("strSearch", medal_name);

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        let maps: OsekaiMaps = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai maps: {body}")
        })?;

        Ok(maps.0.unwrap_or_default())
    }

    pub async fn get_osekai_comments(&self, medal_id: u32) -> Result<Vec<OsekaiComment>> {
        let url = "https://osekai.net/global/api/comment_system.php";

        let form = Multipart::new()
            .push_text("strMedalID", medal_id)
            .push_text("bGetComments", "true");

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
        let form = Multipart::new().push_text("App", R::FORM);

        let bytes = self.make_post_request(url, Site::Osekai, form).await?;

        serde_json::from_slice::<OsekaiRankingEntries<R>>(&bytes)
            .map(Vec::from)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("failed to deserialize Osekai {}: {body}", R::FORM)
            })
    }

    pub async fn get_snipe_player(
        &self,
        country: &str,
        user_id: u32,
    ) -> Result<Option<SnipePlayer>> {
        let url = format!(
            "{HUISMETBENEN}player/{country}/{user_id}?type=id",
            country = country.to_lowercase(),
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        if bytes.as_ref() == b"{}" {
            return Ok(None);
        }

        serde_json::from_slice(&bytes)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("failed to deserialize snipe player: {body}")
            })
            .map(Some)
    }

    pub async fn get_snipe_country(&self, country: &str) -> Result<Vec<SnipeCountryPlayer>> {
        let url = format!(
            "{HUISMETBENEN}rankings/{country}/pp/weighted",
            country = country.to_lowercase()
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
        user_id: u32,
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
            "{HUISMETBENEN}changes/{version}/{user_id}?since={since}&until={until}&includeOwnSnipes=false",
            version = if sniper { "new" } else { "old" },
            since = from.format(DATETIME_FORMAT).unwrap(),
            until = until.format(DATETIME_FORMAT).unwrap()
        );

        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize snipe recent: {body}")
        })
    }

    pub async fn get_national_firsts(&self, params: &SnipeScoreParams) -> Result<Vec<SnipeScore>> {
        let mut url = format!(
            "{HUISMETBENEN}player/{country}/{user}/topranks?sort={sort}&order={order}&page={page}",
            country = params.country,
            user = params.user_id,
            page = params.page,
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
            "{HUISMETBENEN}player/{country}/{user}/topranks/count",
            country = params.country,
            user = params.user_id,
        );

        if let Some(mods) = params.mods {
            if let ModSelection::Include(mods) | ModSelection::Exact(mods) = mods {
                if mods == GameMods::NoMod {
                    url.push_str("?mods=nomod");
                } else {
                    let _ = write!(url, "?mods={mods}");
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
        let mut form = Multipart::new()
            .push_text("rankMin", params.min_rank)
            .push_text("rankMax", params.max_rank)
            .push_text("gamemode", params.mode as u8)
            .push_text("page", params.page);

        if let Some(ref country) = params.country {
            form = form.push_text("country", country);
        }

        let url = "https://osustats.ppy.sh/api/getScoreRanking";
        let post_fut = self.make_post_request(url, Site::OsuStats, form);

        let bytes = match timeout(Duration::from_secs(4), post_fut).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(err)) => return Err(Report::new(err)),
            Err(_) => bail!("timeout while waiting for osustats"),
        };

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
        let mut form = Multipart::new()
            .push_text("accMin", params.min_acc)
            .push_text("accMax", params.max_acc)
            .push_text("rankMin", params.min_rank)
            .push_text("rankMax", params.max_rank)
            .push_text("gamemode", params.mode as u8)
            .push_text("sortBy", params.order as u8)
            .push_text("sortOrder", !params.descending as u8)
            .push_text("page", params.page)
            .push_text("u1", &params.username);

        if let Some(selection) = params.mods {
            let mod_str = match selection {
                ModSelection::Include(mods) => format!("+{mods}"),
                ModSelection::Exclude(mods) => format!("-{mods}"),
                ModSelection::Exact(mods) => format!("!{mods}"),
            };

            form = form.push_text("mods", mod_str);
        }

        let url = "https://osustats.ppy.sh/api/getScores";
        let post_fut = self.make_post_request(url, Site::OsuStats, form);

        let bytes = match timeout(Duration::from_secs(4), post_fut).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(ClientError::BadRequest)) => return Ok((Vec::new(), 0)),
            Ok(Err(err)) => return Err(Report::new(err)),
            Err(_) => bail!("timeout while waiting for osustats"),
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
        self.make_get_request(url, Site::OsuAvatar)
            .await
            .map_err(Report::new)
    }

    pub async fn get_badge(&self, url: &str) -> Result<Bytes> {
        self.make_get_request(url, Site::OsuBadge)
            .await
            .map_err(Report::new)
    }

    /// Make sure you provide a valid url to a mapset cover
    pub async fn get_mapset_cover(&self, cover: &str) -> Result<Bytes> {
        self.make_get_request(&cover, Site::OsuMapsetCover)
            .await
            .map_err(Report::new)
    }

    pub async fn get_map_file(&self, map_id: u32) -> Result<Bytes, ClientError> {
        let url = format!("{OSU_BASE}osu/{map_id}");

        self.make_get_request(&url, Site::OsuMapFile).await
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

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("status code 400 - bad request")]
    BadRequest,
    #[error("status code 404 - not found")]
    NotFound,
    #[error("status code 429 - ratelimited")]
    Ratelimited,
    #[error(transparent)]
    Report(#[from] Report),
}
