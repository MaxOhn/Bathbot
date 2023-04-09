use std::{collections::HashSet, fmt::Write, hash::BuildHasher, time::Duration};

use bathbot_model::{
    OsekaiBadge, OsekaiBadgeOwner, OsekaiComment, OsekaiComments, OsekaiMap, OsekaiMaps,
    OsekaiMedal, OsekaiMedals, OsekaiRanking, OsekaiRankingEntries, OsuStatsParams, OsuStatsPlayer,
    OsuStatsPlayersArgs, OsuStatsScore, OsuStatsScoreVecSeed, OsuTrackerCountryDetails,
    OsuTrackerIdCount, OsuTrackerPpGroup, OsuTrackerStats, RespektiveUser, ScraperScore,
    ScraperScores, SnipeCountryPlayer, SnipeCountryStatistics, SnipePlayer, SnipeRecent,
    SnipeScore, SnipeScoreParams,
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
use rosu_v2::prelude::{mods, GameMod, GameModIntermode, GameMode, GameMods, GameModsIntermode};
use serde::de::DeserializeSeed;
use serde_json::Value;
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

                format!("Failed to deserialize respektive user: {body}")
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

    /// Don't use this; use `RedisCache::osutracker_stats` instead.
    pub async fn get_osutracker_stats(&self) -> Result<OsuTrackerStats> {
        let url = "https://osutracker.com/api/stats";
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker stats: {body}")
        })
    }

    /// Don't use this; use `RedisCache::osutracker_pp_group` instead.
    pub async fn get_osutracker_pp_group(&self, pp: u32) -> Result<OsuTrackerPpGroup> {
        let url = format!("https://osutracker.com/api/stats/ppBarrier?number={pp}");
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker pp groups: {body}")
        })
    }

    /// Don't use this; use `RedisCache::osutracker_counts` instead.
    pub async fn get_osutracker_counts(&self) -> Result<Vec<OsuTrackerIdCount>> {
        let url = "https://osutracker.com/api/stats/idCounts";
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker id counts: {body}")
        })
    }

    /// Don't use this; use `RedisCache::badges` instead.
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

    /// Don't use this; use `RedisCache::medals` instead.
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

            let mut d = serde_json::Deserializer::from(values.next().unwrap());

            let scores = OsuStatsScoreVecSeed::new(params.mode)
                .deserialize(&mut d)
                .wrap_err_with(|| {
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
    pub async fn get_leaderboard<S>(
        &self,
        map_id: u32,
        mods: Option<GameModsIntermode>,
        mode: GameMode,
    ) -> Result<Vec<ScraperScore>>
    where
        S: BuildHasher + Default,
    {
        let mut scores = self._get_leaderboard(map_id, mods).await?;

        let non_mirror = mods
            .map(|mods| !mods.contains(GameModIntermode::Mirror))
            .unwrap_or(true);

        // Check if another request for mania's MR is needed
        if mode == GameMode::Mania && non_mirror {
            let mods = match mods {
                None => Some(mods!(Mirror)),
                Some(mut mods) => Some(mods | GameModIntermode::Mirror),
            };

            let mut new_scores = self._get_leaderboard(map_id, mods).await?;
            scores.append(&mut new_scores);
            scores.sort_unstable_by(|a, b| b.score.cmp(&a.score));
            let mut uniques = HashSet::with_capacity_and_hasher(50, S::default());
            scores.retain(|s| uniques.insert(s.user_id));
            scores.truncate(50);
        }

        // Check if DT / NC is included
        let mods = match mods {
            Some(mods) if mods.contains(GameModIntermode::DoubleTime) => {
                Some(mods | GameModsIntermode::NightCore)
            }
            Some(mods) if mods.contains(GameModIntermode::NightCore) => {
                Some((mods - GameModsIntermode::NightCore) | GameMods::DoubleTime)
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
            let mut uniques = HashSet::with_capacity_and_hasher(50, S::default());
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
