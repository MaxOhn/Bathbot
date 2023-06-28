use std::{borrow::Cow, fmt::Write};

use bathbot_cache::{Cache, CacheSerializer};
use bathbot_model::{
    rosu_v2::ranking::Rankings, OsekaiBadge, OsekaiMedal, OsekaiRanking, OsuStatsBestScores,
    OsuStatsBestTimeframe, OsuTrackerIdCount, OsuTrackerPpGroup, OsuTrackerStats, SnipeCountries,
};
use bathbot_psql::model::osu::MapVersion;
use bathbot_util::{matcher, osu::MapIdType};
use eyre::{Report, Result};
use rkyv::{with::With, Serialize};
use rosu_v2::prelude::{GameMode, OsuError, Rankings as RosuRankings};

pub use self::data::RedisData;
use crate::{commands::osu::MapOrScore, core::Context, util::interaction::InteractionCommand};

pub mod osu;

mod data;

type RedisResult<T, A = T, E = Report> = Result<RedisData<T, A>, E>;

#[derive(Copy, Clone)]
pub struct RedisManager<'c> {
    ctx: &'c Context,
}

impl<'c> RedisManager<'c> {
    pub fn new(ctx: &'c Context) -> Self {
        Self { ctx }
    }

    pub async fn badges(self) -> RedisResult<Vec<OsekaiBadge>> {
        const EXPIRE: usize = 7200;
        const KEY: &str = "osekai_badges";

        let mut conn = match self.ctx.cache.fetch(KEY).await {
            Ok(Ok(badges)) => {
                self.ctx.stats.inc_cached_badges();

                return Ok(RedisData::Archive(badges));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let badges = self.ctx.client().get_osekai_badges().await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store::<_, _, 65_536>(conn, KEY, &badges, EXPIRE).await {
                warn!(?err, "Failed to store badges");
            }
        }

        Ok(RedisData::new(badges))
    }

    pub async fn medals(self) -> RedisResult<Vec<OsekaiMedal>> {
        const EXPIRE: usize = 3600;
        const KEY: &str = "osekai_medals";

        let mut conn = match self.ctx.cache.fetch(KEY).await {
            Ok(Ok(medals)) => {
                self.ctx.stats.inc_cached_medals();

                return Ok(RedisData::Archive(medals));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let medals = self.ctx.client().get_osekai_medals().await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store::<_, _, 16_384>(conn, KEY, &medals, EXPIRE).await {
                warn!(?err, "Failed to store medals");
            }
        }

        Ok(RedisData::new(medals))
    }

    pub async fn osekai_ranking<R>(self) -> RedisResult<Vec<R::Entry>>
    where
        R: OsekaiRanking,
        <R as OsekaiRanking>::Entry: Serialize<CacheSerializer<65_536>>,
    {
        const EXPIRE: usize = 7200;

        let mut key = b"osekai_ranking_".to_vec();
        key.extend_from_slice(R::FORM.as_bytes());

        let mut conn = match self.ctx.cache.fetch(&key).await {
            Ok(Ok(ranking)) => {
                self.ctx.stats.inc_cached_osekai_ranking();

                return Ok(RedisData::Archive(ranking));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let ranking = self.ctx.client().get_osekai_ranking::<R>().await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store::<_, _, 65_536>(conn, &key, &ranking, EXPIRE).await {
                warn!(?err, "Failed to store osekai ranking");
            }
        }

        Ok(RedisData::new(ranking))
    }

    pub async fn osutracker_pp_group(self, pp: u32) -> RedisResult<OsuTrackerPpGroup> {
        const EXPIRE: usize = 86_400;
        let key = format!("osutracker_pp_group_{pp}");

        let mut conn = match self.ctx.cache.fetch(&key).await {
            Ok(Ok(group)) => {
                self.ctx.stats.inc_cached_osutracker_pp_group();

                return Ok(RedisData::Archive(group));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let group = self.ctx.client().get_osutracker_pp_group(pp).await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store::<_, _, 1_024>(conn, &key, &group, EXPIRE).await {
                warn!(?err, "Failed to store osutracker pp group");
            }
        }

        Ok(RedisData::new(group))
    }

    pub async fn osutracker_stats(self) -> RedisResult<OsuTrackerStats> {
        const EXPIRE: usize = 86_400;
        const KEY: &str = "osutracker_stats";

        let mut conn = match self.ctx.cache.fetch(KEY).await {
            Ok(Ok(stats)) => {
                self.ctx.stats.inc_cached_osutracker_stats();

                return Ok(RedisData::Archive(stats));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let stats = self.ctx.client().get_osutracker_stats().await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store::<_, _, 32_768>(conn, KEY, &stats, EXPIRE).await {
                warn!(?err, "Failed to store osutracker stats");
            }
        }

        Ok(RedisData::new(stats))
    }

    pub async fn osutracker_counts(self) -> RedisResult<Vec<OsuTrackerIdCount>> {
        const EXPIRE: usize = 86_400;
        const KEY: &str = "osutracker_id_counts";

        let mut conn = match self.ctx.cache.fetch(KEY).await {
            Ok(Ok(counts)) => {
                self.ctx.stats.inc_cached_osutracker_counts();

                return Ok(RedisData::Archive(counts));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let counts = self.ctx.client().get_osutracker_counts().await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store::<_, _, 1>(conn, KEY, &counts, EXPIRE).await {
                warn!(?err, "Failed to store osutracker counts");
            }
        }

        Ok(RedisData::new(counts))
    }

    pub async fn pp_ranking(
        self,
        mode: GameMode,
        page: u32,
        country: Option<&str>,
    ) -> RedisResult<RosuRankings, Rankings, OsuError> {
        const EXPIRE: usize = 1800;
        let mut key = format!("pp_ranking_{}_{page}", mode as u8);

        if let Some(country) = country {
            let _ = write!(key, "_{country}");
        }

        let mut conn = match self.ctx.cache.fetch(&key).await {
            Ok(Ok(ranking)) => {
                self.ctx.stats.inc_cached_pp_ranking();

                return Ok(RedisData::Archive(ranking));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let ranking_fut = self.ctx.osu().performance_rankings(mode).page(page);

        let ranking = if let Some(country) = country {
            ranking_fut.country(country).await?
        } else {
            ranking_fut.await?
        };

        if let Some(ref mut conn) = conn {
            let with = With::<_, Rankings>::cast(&ranking);

            if let Err(err) = Cache::store::<_, _, 32_768>(conn, &key, with, EXPIRE).await {
                warn!(?err, "Failed to store ranking");
            }
        }

        Ok(RedisData::new(ranking))
    }

    pub async fn osustats_best(
        self,
        timeframe: OsuStatsBestTimeframe,
        mode: GameMode,
    ) -> RedisResult<OsuStatsBestScores> {
        const EXPIRE: usize = 3600;
        let key = format!("osustats_best_{}_{}", timeframe as u8, mode as u8);

        let mut conn = match self.ctx.cache.fetch(&key).await {
            Ok(Ok(scores)) => {
                self.ctx.stats.inc_cached_osustats_best();

                return Ok(RedisData::Archive(scores));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let scores = self.ctx.client().get_osustats_best(timeframe, mode).await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store::<_, _, 8192>(conn, &key, &scores, EXPIRE).await {
                warn!(?err, "Failed to store osustats best");
            }
        }

        Ok(RedisData::new(scores))
    }

    pub async fn snipe_countries(self) -> RedisResult<SnipeCountries> {
        const EXPIRE: usize = 43_200; // 12 hours
        let key = "snipe_countries";

        let mut conn = match self.ctx.cache.fetch(key).await {
            Ok(Ok(countries)) => {
                self.ctx.stats.inc_cached_snipe_countries();

                return Ok(RedisData::Archive(countries));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let countries = self.ctx.client().get_snipe_countries().await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store::<_, _, 712>(conn, key, &countries, EXPIRE).await {
                warn!(?err, "Failed to store snipe countries");
            }
        }

        Ok(RedisData::new(countries))
    }

    // Mapset difficulty names for the autocomplete option of the compare command
    pub async fn cs_diffs(
        self,
        command: &InteractionCommand,
        map: &Option<Cow<'_, str>>,
        idx: Option<u32>,
    ) -> RedisResult<Vec<MapVersion>> {
        const EXPIRE: usize = 30;

        let idx = match idx {
            Some(idx @ 0..=50) => idx.saturating_sub(1) as usize,
            // Invalid index, ignore
            Some(_) => return Ok(RedisData::new(Vec::new())),
            None => 0,
        };

        let map_ = map.as_deref().unwrap_or_default();
        let key = format!("diffs_{}_{idx}_{map_}", command.id);

        let mut conn = match self.ctx.cache.fetch(&key).await {
            Ok(Ok(diffs)) => {
                self.ctx.stats.inc_cached_cs_diffs();

                return Ok(RedisData::Archive(diffs));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let map = if let Some(map) = map {
            if let Some(id) = matcher::get_osu_map_id(map)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(map).map(MapIdType::Set))
            {
                Some(MapOrScore::Map(id))
            } else if let Some((mode, id)) = matcher::get_osu_score_id(map) {
                Some(MapOrScore::Score { mode, id })
            } else {
                // Invalid map input, ignore
                return Ok(RedisData::new(Vec::new()));
            }
        } else {
            None
        };

        let map_id = match map {
            Some(MapOrScore::Map(id)) => Some(id),
            Some(MapOrScore::Score { id, mode }) => match self.ctx.osu().score(id, mode).await {
                Ok(score) => Some(MapIdType::Map(score.map_id)),
                Err(err) => return Err(Report::new(err).wrap_err("Failed to get score")),
            },
            None => match self.ctx.retrieve_channel_history(command.channel_id).await {
                Ok(msgs) => self.ctx.find_map_id_in_msgs(&msgs, idx).await,
                Err(err) => return Err(err.wrap_err("Failed to retrieve channel history")),
            },
        };

        let diffs = match map_id {
            Some(MapIdType::Map(map_id)) => self.ctx.osu_map().versions_by_map(map_id).await?,
            Some(MapIdType::Set(mapset_id)) => {
                self.ctx.osu_map().versions_by_mapset(mapset_id).await?
            }
            None => Vec::new(),
        };

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store::<_, _, 64>(conn, &key, &diffs, EXPIRE).await {
                warn!(?err, "Failed to store cs diffs");
            }
        }

        Ok(RedisData::new(diffs))
    }
}
