use std::{borrow::Cow, fmt::Write};

use bathbot_cache::Cache;
use bathbot_model::{
    rosu_v2::ranking::RankingsRkyv, ArchivedOsuStatsBestScores, OsekaiBadge, OsekaiMedal,
    OsekaiRanking, OsuStatsBestScores, OsuStatsBestTimeframe, SnipeCountries,
};
use bathbot_psql::model::osu::{ArchivedMapVersion, MapVersion};
use bathbot_util::{matcher, osu::MapIdType};
use eyre::{Report, Result, WrapErr};
use rkyv::{
    bytecheck::CheckBytes,
    rancor::{BoxedError, Panic, Strategy},
    ser::{allocator::ArenaHandle, Serializer},
    util::AlignedVec,
    validation::{archive::ArchiveValidator, Validator},
    vec::ArchivedVec,
    with::With,
    Archive, Serialize,
};
use rosu_v2::prelude::{GameMode, OsuError, Rankings};

pub use self::data::RedisData;
use crate::{
    core::{BotMetrics, Context},
    util::{
        cached_archive::{serialize_using_arena, CachedArchive},
        interaction::InteractionCommand,
        osu::MapOrScore,
    },
};

pub mod osu;

mod data;

type RedisResult<T, A = T, E = Report> = Result<RedisData<T, A>, E>;

#[derive(Copy, Clone)]
pub struct RedisManager;

impl RedisManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn badges(self) -> RedisResult<Vec<OsekaiBadge>> {
        const EXPIRE: u64 = 7200;
        const KEY: &str = "osekai_badges";

        let mut conn = match Context::cache().fetch(KEY).await {
            Ok(Ok(badges)) => {
                BotMetrics::inc_redis_hit("Osekai badges");

                return Ok(RedisData::Archive(badges));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let badges = Context::client().get_osekai_badges().await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, KEY, &badges, EXPIRE).await {
                warn!(?err, "Failed to store badges");
            }
        }

        Ok(RedisData::new(badges))
    }

    pub async fn medals(self) -> RedisResult<Vec<OsekaiMedal>> {
        const EXPIRE: u64 = 3600;
        const KEY: &str = "osekai_medals";

        let mut conn = match Context::cache().fetch(KEY).await {
            Ok(Ok(medals)) => {
                BotMetrics::inc_redis_hit("Osekai medals");

                return Ok(RedisData::Archive(medals));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let medals = Context::client().get_osekai_medals().await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, KEY, &medals, EXPIRE).await {
                warn!(?err, "Failed to store medals");
            }
        }

        Ok(RedisData::new(medals))
    }

    pub async fn osekai_ranking<R>(self) -> RedisResult<Vec<R::Entry>>
    where
        R: OsekaiRanking,
        <R as OsekaiRanking>::Entry: for<'a> Serialize<Strategy<Serializer<AlignedVec<8>, ArenaHandle<'a>, ()>, BoxedError>>
            + for<'a> Archive<
                Archived: CheckBytes<Strategy<Validator<ArchiveValidator<'a>, ()>, Panic>>,
            >,
    {
        const EXPIRE: u64 = 7200;

        let mut key = b"osekai_ranking_".to_vec();
        key.extend_from_slice(R::FORM.as_bytes());

        let mut conn = match Context::cache().fetch(&key).await {
            Ok(Ok(ranking)) => {
                BotMetrics::inc_redis_hit("Osekai ranking");

                return Ok(RedisData::Archive(ranking));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let ranking = Context::client().get_osekai_ranking::<R>().await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, &key, &ranking, EXPIRE).await {
                warn!(?err, "Failed to store osekai ranking");
            }
        }

        Ok(RedisData::new(ranking))
    }

    pub async fn pp_ranking(
        self,
        mode: GameMode,
        page: u32,
        country: Option<&str>,
    ) -> RedisResult<Rankings, Rankings, OsuError> {
        const EXPIRE: u64 = 1800;
        let mut key = format!("pp_ranking_{}_{page}", mode as u8);

        if let Some(country) = country {
            let _ = write!(key, "_{country}");
        }

        let mut conn = match Context::cache()
            .fetch_with::<_, _, RankingsRkyv>(&key)
            .await
        {
            Ok(Ok(ranking)) => {
                BotMetrics::inc_redis_hit("PP ranking");

                return Ok(RedisData::Archive(ranking));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let ranking_fut = Context::osu().performance_rankings(mode).page(page);

        let ranking = if let Some(country) = country {
            ranking_fut.country(country).await?
        } else {
            ranking_fut.await?
        };

        if let Some(ref mut conn) = conn {
            let with = With::<_, RankingsRkyv>::cast(&ranking);

            if let Err(err) = Cache::store(conn, &key, with, EXPIRE).await {
                warn!(?err, "Failed to store ranking");
            }
        }

        Ok(RedisData::new(ranking))
    }

    pub async fn osustats_best(
        self,
        timeframe: OsuStatsBestTimeframe,
        mode: GameMode,
    ) -> Result<CachedArchive<ArchivedOsuStatsBestScores>> {
        const EXPIRE: u64 = 3600;
        let key = format!("osustats_best_{}_{}", timeframe as u8, mode as u8);

        let mut conn = match Context::cache().fetch::<_, OsuStatsBestScores>(&key).await {
            Ok(Ok(scores)) => {
                BotMetrics::inc_redis_hit("osu!stats best");

                return CachedArchive::new(scores.into_bytes()).wrap_err("Failed validation");
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let scores = Context::client().get_osustats_best(timeframe, mode).await?;

        let bytes = serialize_using_arena(&scores)
            .wrap_err_with(|| format!("Failed to serialize key {key}"))?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store_raw(conn, &key, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store osustats best");
            }
        }

        CachedArchive::new(bytes).wrap_err("Failed validation")
    }

    pub async fn snipe_countries(self, mode: GameMode) -> RedisResult<SnipeCountries> {
        const EXPIRE: u64 = 43_200; // 12 hours
        let key = format!("snipe_countries_{mode}");

        let mut conn = match Context::cache().fetch(&key).await {
            Ok(Ok(countries)) => {
                BotMetrics::inc_redis_hit("Snipe countries");

                return Ok(RedisData::Archive(countries));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let countries = Context::client().get_snipe_countries(mode).await?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, &key, &countries, EXPIRE).await {
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
    ) -> Result<Option<CachedArchive<ArchivedVec<ArchivedMapVersion>>>> {
        const EXPIRE: u64 = 30;

        let idx = match idx {
            Some(idx @ 0..=50) => idx.saturating_sub(1) as usize,
            // Invalid index, ignore
            Some(_) => return Ok(None),
            None => 0,
        };

        let map_ = map.as_deref().unwrap_or_default();
        let key = format!("diffs_{}_{idx}_{map_}", command.id);

        let mut conn = match Context::cache().fetch::<_, Vec<MapVersion>>(&key).await {
            Ok(Ok(diffs)) => {
                BotMetrics::inc_redis_hit("Beatmap difficulties");

                return CachedArchive::new(diffs.into_bytes())
                    .map(Some)
                    .wrap_err("Failed validation");
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
            } else if let Some((id, mode)) = matcher::get_osu_score_id(map) {
                Some(MapOrScore::Score { id, mode })
            } else {
                // Invalid map input, ignore
                return Ok(None);
            }
        } else {
            None
        };

        let map_id = match map {
            Some(MapOrScore::Map(id)) => Some(id),
            Some(MapOrScore::Score { id, mode }) => {
                let mut score_fut = Context::osu().score(id);

                if let Some(mode) = mode {
                    score_fut = score_fut.mode(mode);
                }

                match score_fut.await {
                    Ok(score) => Some(MapIdType::Map(score.map_id)),
                    Err(err) => return Err(Report::new(err).wrap_err("Failed to get score")),
                }
            }
            None => match Context::retrieve_channel_history(command.channel_id).await {
                Ok(msgs) => Context::find_map_id_in_msgs(&msgs, idx).await,
                Err(err) => return Err(err.wrap_err("Failed to retrieve channel history")),
            },
        };

        let diffs = match map_id {
            Some(MapIdType::Map(map_id)) => Context::osu_map().versions_by_map(map_id).await?,
            Some(MapIdType::Set(mapset_id)) => {
                Context::osu_map().versions_by_mapset(mapset_id).await?
            }
            None => Vec::new(),
        };

        let bytes = serialize_using_arena(&diffs)
            .wrap_err_with(|| format!("Failed to serialize key {key}"))?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store_raw(conn, &key, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store cs diffs");
            }
        }

        CachedArchive::new(bytes)
            .map(Some)
            .wrap_err("Failed validation")
    }
}
