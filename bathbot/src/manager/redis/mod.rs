use std::{borrow::Cow, fmt::Write, future::Future};

use bathbot_cache::{
    bathbot::map_diffs::CacheMapDiffs,
    data::{BathbotRedisData, BathbotRedisSerializer, BathbotRedisValidator},
    huismetbenen::snipe_countries::CacheSnipeCountries,
    osekai::{badges::CacheBadges, medals::CacheMedals, ranking::CacheOsekaiRanking},
    osu::pp_ranking::CachePpRanking,
    osu_world::country_regions::CacheCountryRegions,
    osustats::best::CacheOsuStatsBest,
    value::CachedArchive,
};
use bathbot_client::Client;
use bathbot_model::{
    rosu_v2::ranking::ArchivedRankings, CountryRegions, OsekaiBadge, OsekaiMedal, OsekaiRanking,
    OsuStatsBestScores, OsuStatsBestTimeframe, SnipeCountries,
};
use bathbot_psql::model::osu::MapVersion;
use bathbot_util::{matcher, osu::MapIdType};
use eyre::{Report, Result, WrapErr};
use rkyv::{bytecheck::CheckBytes, util::AlignedVec, Archived, Serialize};
use rosu_v2::prelude::GameMode;

use crate::{
    commands::osu::MapOrScore,
    core::{BotMetrics, Context},
    util::interaction::InteractionCommand,
};

pub mod osu;

type RedisResult<T> = Result<CachedArchive<Archived<T>>>;

#[derive(Copy, Clone)]
pub struct RedisManager;

impl RedisManager {
    pub fn new() -> Self {
        Self
    }

    async fn fetch<T, R, F, U, C>(
        self,
        key: &str,
        metrics_hit: &'static str,
        request: R,
        convert: C,
    ) -> Result<CachedArchive<T::Archived>>
    where
        T: BathbotRedisData,
        R: FnOnce(&'static Client) -> F,
        F: Future<Output = Result<U>>,
        C: for<'a> FnOnce(&'a U) -> &'a T::Original,
    {
        match Context::cache().fetch::<T>(key).await {
            Ok(Some(data)) => {
                BotMetrics::inc_redis_hit(metrics_hit);

                return Ok(data);
            }
            Ok(None) => {}
            Err(err) => warn!("{err:?}"),
        }

        let data = request(Context::client()).await?;

        let bytes =
            T::serialize(convert(&data)).wrap_err_with(|| format!("Failed to serialize {key}"))?;

        let store_fut = Context::cache().store_serialized::<T>(key, bytes.as_slice());

        if let Err(err) = store_fut.await {
            warn!(?err, "Failed to store {key}");
        }

        CachedArchive::new(bytes)
    }

    pub async fn badges(self) -> RedisResult<Vec<OsekaiBadge>> {
        self.fetch::<CacheBadges, _, _, _, _>(
            "OSEKAI_BADGES",
            "Osekai badges",
            Client::get_osekai_badges,
            AsRef::as_ref,
        )
        .await
    }

    pub async fn medals(self) -> RedisResult<Vec<OsekaiMedal>> {
        self.fetch::<CacheMedals, _, _, _, _>(
            "OSEKAI_MEDALS",
            "Osekai medals",
            Client::get_osekai_medals,
            AsRef::as_ref,
        )
        .await
    }

    pub async fn osekai_ranking<R>(self) -> RedisResult<Vec<R::Entry>>
    where
        R: OsekaiRanking,
        R::Entry: for<'a> Serialize<
            BathbotRedisSerializer<'a>,
            Archived: CheckBytes<BathbotRedisValidator<'a>>,
        >,
    {
        let mut key = "OSEKAI_RANKING_".to_string();
        key.push_str(R::FORM);

        self.fetch::<CacheOsekaiRanking<R>, _, _, _, _>(
            &key,
            "Osekai ranking",
            Client::get_osekai_ranking::<R>,
            AsRef::as_ref,
        )
        .await
    }

    pub async fn pp_ranking(
        self,
        mode: GameMode,
        page: u32,
        country: Option<&str>,
    ) -> Result<CachedArchive<ArchivedRankings>> {
        let mut key = format!("PP_RANKING_{}_{page}", mode as u8);

        if let Some(country) = country {
            let _ = write!(key, "_{country}");
        }

        self.fetch::<CachePpRanking, _, _, _, _>(
            &key,
            "PP ranking",
            |_| async {
                let ranking_fut = Context::osu().performance_rankings(mode).page(page);

                let res = if let Some(country) = country {
                    ranking_fut.country(country).await
                } else {
                    ranking_fut.await
                };

                res.map_err(Report::new)
            },
            |value| value,
        )
        .await
    }

    pub async fn osustats_best(
        self,
        timeframe: OsuStatsBestTimeframe,
        mode: GameMode,
    ) -> RedisResult<OsuStatsBestScores> {
        let key = format!("OSUSTATS_BEST_{}_{}", timeframe as u8, mode as u8);

        self.fetch::<CacheOsuStatsBest, _, _, _, _>(
            &key,
            "osu!stats best",
            |client| client.get_osustats_best(timeframe, mode),
            |value| value,
        )
        .await
    }

    pub async fn snipe_countries(self, mode: GameMode) -> RedisResult<SnipeCountries> {
        let key = format!("SNIPE_COUNTRIES_{mode}");

        self.fetch::<CacheSnipeCountries, _, _, _, _>(
            &key,
            "Snipe countries",
            |client| client.get_snipe_countries(mode),
            |value| value,
        )
        .await
    }

    pub async fn country_regions(self) -> RedisResult<CountryRegions> {
        self.fetch::<CacheCountryRegions, _, _, _, _>(
            "COUNTRY_REGIONS",
            "Country regions",
            Client::get_country_regions,
            |value| value,
        )
        .await
    }

    // Mapset difficulty names for the autocomplete option of the compare command
    pub async fn cs_diffs(
        self,
        command: &InteractionCommand,
        map: &Option<Cow<'_, str>>,
        idx: Option<u32>,
    ) -> RedisResult<Vec<MapVersion>> {
        fn serialize(key: &str, diffs: &[MapVersion]) -> Result<AlignedVec<8>> {
            CacheMapDiffs::serialize(&diffs).wrap_err_with(|| format!("Failed to serialize {key}"))
        }

        let idx = match idx {
            Some(idx @ 0..=50) => idx.saturating_sub(1) as usize,
            // Invalid index, ignore
            Some(_) => return serialize("EMPTY_VEC", &[]).and_then(CachedArchive::new),
            None => 0,
        };

        let map_ = map.as_deref().unwrap_or_default();
        let key = format!("DIFFS_{}_{idx}_{map_}", command.id);

        match Context::cache().fetch::<CacheMapDiffs>(&key).await {
            Ok(Some(data)) => {
                BotMetrics::inc_redis_hit("Beatmap difficulties");

                return Ok(data);
            }
            Ok(None) => {}
            Err(err) => warn!("{err:?}"),
        }

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
                return serialize("EMPTY_VEC", &[]).and_then(CachedArchive::new);
            }
        } else {
            None
        };

        let map_id = match map {
            Some(MapOrScore::Map(id)) => Some(id),
            Some(MapOrScore::Score { id, mode }) => match Context::osu().score(id).mode(mode).await
            {
                Ok(score) => Some(MapIdType::Map(score.map_id)),
                Err(err) => return Err(Report::new(err).wrap_err("Failed to get score")),
            },
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

        let bytes = serialize(&key, &diffs)?;

        let store_fut = Context::cache().store_serialized::<CacheMapDiffs>(&key, bytes.as_slice());

        if let Err(err) = store_fut.await {
            warn!(?err, "Failed to store {key}");
        }

        CachedArchive::new(bytes)
    }
}
