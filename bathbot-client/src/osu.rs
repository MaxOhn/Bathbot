use std::{
    fmt::{Formatter, Result as FmtResult, Write},
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD, Engine};
use bathbot_model::{
    ModeAsSeed, OsekaiBadge, OsekaiBadgeOwner, OsekaiComment, OsekaiComments, OsekaiMap,
    OsekaiMaps, OsekaiMedal, OsekaiRanking, OsekaiRankingEntries, OsuStatsBestScores,
    OsuStatsBestTimeframe, OsuStatsParams, OsuStatsPlayer, OsuStatsPlayersArgs, OsuStatsScoresRaw,
    OsuTrackerCountryDetails, OsuTrackerIdCount, OsuTrackerPpGroup, OsuTrackerStats,
    RespektiveUser, RespektiveUsers, SnipeCountries, SnipeCountryPlayer, SnipeCountryStatistics,
    SnipePlayer, SnipeRecent, SnipeScore, SnipeScoreParams,
};
use bathbot_util::{
    constants::{HUISMETBENEN, OSU_BASE},
    datetime::{DATE_FORMAT, TIME_FORMAT},
    osu::ModSelection,
};
use bytes::Bytes;
use eyre::{Report, Result, WrapErr};
use http::{header::USER_AGENT, Method, Request, Response};
use hyper::Body;
use rosu_v2::prelude::GameMode;
use serde::{
    de::{DeserializeSeed, Error as DeError, Visitor},
    Deserialize, Deserializer,
};
use time::{format_description::FormatItem, OffsetDateTime};
use tokio::time::timeout;

use crate::{multipart::Multipart, Client, ClientError, Site, MY_USER_AGENT};

impl Client {
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

    pub async fn get_respektive_users(
        &self,
        user_ids: impl IntoIterator<Item = u32>,
        mode: GameMode,
    ) -> Result<RespektiveUsers> {
        let mut url = "https://score.respektive.pw/u/".to_owned();

        let mut user_ids = user_ids.into_iter();

        let user_id = user_ids.next().expect("require at least one user id");
        let _ = write!(url, "{}", user_id);

        for user_id in user_ids {
            let _ = write!(url, ",{}", user_id);
        }

        let _ = write!(url, "?m={}", mode as u8);

        let bytes = self.make_get_request(url, Site::Respektive).await?;

        let users: Vec<RespektiveUser> = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize respektive user: {body}")
        })?;

        Ok(users.into())
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

                format!("Failed to deserialize respektive rank: {body}")
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

    /// Don't use this; use `RedisManager::osutracker_stats` instead.
    pub async fn get_osutracker_stats(&self) -> Result<OsuTrackerStats> {
        let url = "https://osutracker.com/api/stats";
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker stats: {body}")
        })
    }

    /// Don't use this; use `RedisManager::osutracker_pp_group` instead.
    pub async fn get_osutracker_pp_group(&self, pp: u32) -> Result<OsuTrackerPpGroup> {
        let url = format!("https://osutracker.com/api/stats/ppBarrier?number={pp}");
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker pp groups: {body}")
        })
    }

    /// Don't use this; use `RedisManager::osutracker_counts` instead.
    pub async fn get_osutracker_counts(&self) -> Result<Vec<OsuTrackerIdCount>> {
        let url = "https://osutracker.com/api/stats/idCounts";
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker id counts: {body}")
        })
    }

    /// Don't use this; use `RedisManager::badges` instead.
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

    /// Don't use this; use `RedisManager::medals` instead.
    pub async fn get_osekai_medals(&self) -> Result<Vec<OsekaiMedal>> {
        let url = "https://osekai.net/medals/api/medals.php";
        let form = Multipart::new().push_text("strSearch", "");

        let bytes = self
            .make_multipart_post_request(url, Site::Osekai, form)
            .await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize osekai medals: {body}")
        })
    }

    pub async fn get_osekai_beatmaps(&self, medal_name: &str) -> Result<Vec<OsekaiMap>> {
        let url = "https://osekai.net/medals/api/beatmaps.php";
        let form = Multipart::new().push_text("strSearch", medal_name);

        let bytes = self
            .make_multipart_post_request(url, Site::Osekai, form)
            .await?;

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

        let bytes = self
            .make_multipart_post_request(url, Site::Osekai, form)
            .await?;

        let comments: OsekaiComments = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai comments: {body}")
        })?;

        Ok(comments.0.unwrap_or_default())
    }

    /// Don't use this; use `RedisManager::osekai_ranking` instead.
    pub async fn get_osekai_ranking<R: OsekaiRanking>(&self) -> Result<Vec<R::Entry>> {
        let url = "https://osekai.net/rankings/api/api.php";
        let form = Multipart::new().push_text("App", R::FORM);

        let bytes = self
            .make_multipart_post_request(url, Site::Osekai, form)
            .await?;

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

        if let Some(ref mods) = params.mods {
            if let ModSelection::Include(mods) | ModSelection::Exact(mods) = mods {
                if mods.is_empty() {
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

        if let Some(ref mods) = params.mods {
            if let ModSelection::Include(mods) | ModSelection::Exact(mods) = mods {
                if mods.is_empty() {
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

    /// Don't use this; use `RedisManager::snipe_countries` instead.
    pub async fn get_snipe_countries(&self) -> Result<SnipeCountries> {
        let url = "https://api.huismetbenen.nl/country/all?only_with_data=true";
        let bytes = self.make_get_request(url, Site::Huismetbenen).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize snipe countries: {body}")
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
        let post_fut = self.make_multipart_post_request(url, Site::OsuStats, form);

        let bytes = match timeout(Duration::from_secs(4), post_fut).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(err)) => return Err(Report::new(err)),
            Err(_) => bail!("Timeout while waiting for osustats players"),
        };

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize osustats players: {body}")
        })
    }

    /// Be sure whitespaces in the username are **not** replaced
    pub async fn get_global_scores(&self, params: &OsuStatsParams) -> Result<OsuStatsScoresRaw> {
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

        if let Some(ref selection) = params.mods {
            let mod_str = match selection {
                ModSelection::Include(mods) if mods.is_empty() => "!NM".to_owned(),
                ModSelection::Include(mods) => format!("+{mods}"),
                ModSelection::Exclude(mods) => format!("-{mods}"),
                ModSelection::Exact(mods) => format!("!{mods}"),
            };

            form = form.push_text("mods", mod_str);
        }

        let url = "https://osustats.ppy.sh/api/getScores";
        let post_fut = self.make_multipart_post_request(url, Site::OsuStats, form);

        let bytes = match timeout(Duration::from_secs(4), post_fut).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(ClientError::BadRequest)) => Bytes::from_static(b"[[],0,true,true]"),
            Ok(Err(err)) => return Err(Report::new(err)),
            Err(_) => bail!("Timeout while waiting for osustats scores"),
        };

        Ok(OsuStatsScoresRaw::new(params.mode, bytes.into()))
    }

    /// Don't use this; use `RedisManager::osustats_best` instead.
    pub async fn get_osustats_best(
        &self,
        timeframe: OsuStatsBestTimeframe,
        mode: GameMode,
    ) -> Result<OsuStatsBestScores> {
        let form = Multipart::new()
            .push_text("gamemode", mode as u8)
            .push_text("amount", 100)
            .push_text("duration", timeframe as u8);

        let url = "https://osustats.ppy.sh/api/getBestDayScores";
        let post_fut = self.make_multipart_post_request(url, Site::OsuStats, form);

        let bytes = match timeout(Duration::from_secs(15), post_fut).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(err)) => return Err(Report::new(err)),
            Err(_) => bail!("Timeout while waiting for osustats recentbest"),
        };

        let mut deserializer = serde_json::Deserializer::from_slice(&bytes);

        ModeAsSeed::<OsuStatsBestScores>::new(mode)
            .deserialize(&mut deserializer)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize osustats recentbest: {body}")
            })
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

    pub async fn get_raw_osu_replay(&self, key: &str, score_id: u64) -> Result<Option<Box<[u8]>>> {
        #[derive(Deserialize)]
        struct RawReplayBody {
            #[serde(default, rename = "content", deserialize_with = "decode_base64")]
            decoded: Option<Box<[u8]>>,
        }

        fn decode_base64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Box<[u8]>>, D::Error> {
            struct RawReplayVisitor;

            impl<'de> Visitor<'de> for RawReplayVisitor {
                type Value = Box<[u8]>;

                fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                    f.write_str("a base64 encoded string")
                }

                fn visit_str<E: DeError>(self, v: &str) -> Result<Self::Value, E> {
                    STANDARD
                        .decode(v)
                        .map(Vec::into_boxed_slice)
                        .map_err(|e| DeError::custom(format!("Failed to decode base64: {e}")))
                }
            }

            d.deserialize_str(RawReplayVisitor).map(Some)
        }

        let url = format!("https://osu.ppy.sh/api/get_replay?k={key}&s={score_id}");

        let bytes = self.make_get_request(url, Site::OsuReplay).await?;

        let RawReplayBody { decoded } = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize replay body: {body}")
        })?;

        Ok(decoded)
    }
}
