use std::{borrow::Cow, collections::HashMap, fmt::Write};

use bathbot_cache::{
    Cache,
    model::{CachedArchive, ValidatorStrategy},
    util::serialize::{SerializerStrategy, serialize_using_arena, serialize_using_arena_and_with},
};
use bathbot_model::{
    ArchivedOsekaiBadge, ArchivedOsekaiMedal, ArchivedOsuStatsBestScores,
    ArchivedOsuTrackHistoryEntry, ArchivedScrapedMedal, ArchivedSnipeCountries, OsekaiRanking,
    OsuStatsBestScores, OsuStatsBestTimeframe,
    rosu_v2::{
        multiplayer::{ArchivedRoom, RoomRkyv},
        ranking::{ArchivedRankings, RankingsRkyv},
    },
};
use bathbot_psql::model::osu::MapVersion;
use bathbot_util::{IntHasher, matcher, osu::MapIdType};
use eyre::{ContextCompat, Report, Result, WrapErr};
use futures::{StreamExt, stream::FuturesUnordered};
use rkyv::{
    Archived, Serialize, bytecheck::CheckBytes, collections::swiss_table::ArchivedHashMap,
    primitive::ArchivedU16, rancor::BoxedError, vec::ArchivedVec,
};
use rosu_v2::{
    prelude::{GameMode, RoomCategory},
    request::RoomsFilter,
};
use thiserror::Error as ThisError;
use time::{Date, UtcDateTime};

use crate::{
    core::{BotMetrics, Context},
    util::{interaction::InteractionCommand, osu::MapOrScore},
};

pub mod osu;

// type RedisResult<T, A = T, E = Report> = Result<RedisData<T, A>, E>;
type RedisResult<T> = Result<CachedArchive<T>, RedisError>;

#[derive(Debug, ThisError)]
pub enum RedisError {
    #[error("Failed to acquire data")]
    Acquire(#[from] Report),
    #[error("Failed to serialize data")]
    Serialization(#[source] BoxedError),
    #[error("Failed to validate data")]
    Validation(#[source] BoxedError),
}

#[derive(Copy, Clone)]
pub struct RedisManager;

impl RedisManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn badges(self) -> RedisResult<ArchivedVec<ArchivedOsekaiBadge>> {
        const EXPIRE: u64 = 7200;
        const KEY: &str = "osekai_badges";

        let mut conn = match Context::cache().fetch(KEY).await {
            Ok(Ok(badges)) => {
                BotMetrics::inc_redis_hit("Osekai badges");

                return Ok(badges);
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!(?err, "Failed to fetch osekai badges");

                None
            }
        };

        let badges = Context::client().get_osekai_badges().await?;

        let bytes = serialize_using_arena(&badges).map_err(RedisError::Serialization)?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, KEY, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store badges");
            }
        }

        CachedArchive::new(bytes).map_err(RedisError::Validation)
    }

    pub async fn medals(self) -> RedisResult<ArchivedVec<ArchivedOsekaiMedal>> {
        const EXPIRE: u64 = 3600;
        const KEY: &str = "osekai_medals";

        let mut conn = match Context::cache().fetch(KEY).await {
            Ok(Ok(medals)) => {
                BotMetrics::inc_redis_hit("Osekai medals");

                return Ok(medals);
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!(?err, "Failed to fetch osekai medals");

                None
            }
        };

        let medals = Context::client().get_osekai_medals().await?;

        let bytes = serialize_using_arena(&medals).map_err(RedisError::Serialization)?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, KEY, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store medals");
            }
        }

        CachedArchive::new(bytes).map_err(RedisError::Validation)
    }

    pub async fn medal_icons(self, medal_ids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>> {
        async fn scraped_medals(
            force_request: bool,
        ) -> RedisResult<ArchivedHashMap<ArchivedU16, ArchivedScrapedMedal>> {
            const EXPIRE: u64 = 3600;
            const KEY: &str = "scraped_medals";

            let mut conn = if force_request {
                None
            } else {
                match Context::cache().fetch(KEY).await {
                    Ok(Ok(medals)) => {
                        BotMetrics::inc_redis_hit("Scraped medals");

                        return Ok(medals);
                    }
                    Ok(Err(conn)) => Some(conn),
                    Err(err) => {
                        warn!(?err, "Failed to fetch scraped medals");

                        None
                    }
                }
            };

            let medals: HashMap<_, _, IntHasher> = Context::client()
                .get_medals()
                .await?
                .into_iter()
                .map(|medal| (medal.id, medal))
                .collect();

            let bytes = serialize_using_arena(&medals).map_err(RedisError::Serialization)?;

            let mut store_bytes = async || {
                if let Some(ref mut conn) = conn {
                    Some(Cache::store(conn, KEY, bytes.as_slice(), EXPIRE).await)
                } else if force_request {
                    Some(
                        Context::cache()
                            .store_new(KEY, bytes.as_slice(), EXPIRE)
                            .await,
                    )
                } else {
                    None
                }
            };

            if let Some(Err(err)) = store_bytes().await {
                warn!(?err, "Failed to store scraped medals");
            }

            CachedArchive::new(bytes).map_err(RedisError::Validation)
        }

        let mut medals = scraped_medals(false).await?;
        let mut re_requested = false;

        let mut medal_icons = Vec::new();
        let mut remaining_medals = FuturesUnordered::new();

        let cache = Context::cache();
        let client = Context::client();

        let medal_icon_key = |medal_id| format!("medal_icon_{medal_id}");

        for &medal_id in medal_ids {
            let medal = match medals.get(&(medal_id as u16).into()) {
                Some(medal) => medal,
                None if re_requested => bail!("unknown medal id {medal_id}"),
                None => {
                    medals = scraped_medals(true).await?;
                    re_requested = true;

                    match medals.get(&(medal_id as u16).into()) {
                        Some(medal) => medal,
                        None => bail!("unknown medal id {medal_id}"),
                    }
                }
            };

            let key = medal_icon_key(medal_id);

            // Instead of using the redis cache they could also be stored in
            // the database. That way they wouldn't be removed on each cold
            // reboot. TODO: create DB table
            let medal_icon_opt = match cache.fetch_raw(&key).await {
                Ok(Ok(bytes)) => Some(bytes),
                Ok(Err(_)) => None,
                Err(err) => {
                    warn!(?err, "Failed to fetch scraped medals");

                    None
                }
            };

            match medal_icon_opt {
                Some(bytes) => {
                    medal_icons.push((medal_id, bytes));
                }
                None => {
                    let icon_url = Box::<str>::from(medal.icon_url.as_ref());
                    remaining_medals
                        .push(async move { (medal_id, client.get_medal_icon(&icon_url).await) });
                }
            }
        }

        if remaining_medals.is_empty() {
            debug!("Found all {} medal icons in cache", medal_ids.len());
        } else {
            debug!(
                "Fetching the remaining {}/{} medal icons...",
                remaining_medals.len(),
                medal_ids.len()
            );
        }

        while let Some((medal_id, res)) = remaining_medals.next().await {
            let bytes =
                res.wrap_err_with(|| format!("Failed to fetch icon for medal id {medal_id}"))?;

            if let Err(err) = cache.store_forever(&medal_icon_key(medal_id), &bytes).await {
                warn!(?err, "Failed to store icon of medal {medal_id}");
            }

            medal_icons.push((medal_id, bytes.into()));
        }

        Ok(medal_icons)
    }

    pub async fn daily_challenge(self, date: Date) -> RedisResult<ArchivedRoom> {
        let now = UtcDateTime::now();
        let is_today = now.date() == date;

        let key_fn = |date: Date| {
            format!(
                "daily_challenge_{}_{}_{}",
                date.year() % 2000,
                date.month() as u8,
                date.day()
            )
        };

        let key = key_fn(date);

        match Context::cache().fetch(&key).await {
            Ok(Ok(room)) => {
                BotMetrics::inc_redis_hit("Daily challenge room");

                return Ok(room);
            }
            Ok(Err(_)) => {}
            Err(err) => warn!(?err, "Failed to fetch daily challenge room"),
        };

        let filter = if is_today {
            RoomsFilter::Active
        } else {
            RoomsFilter::Ended
        };

        let rooms = Context::osu()
            .rooms()
            .category(RoomCategory::DailyChallenge)
            .filter(filter)
            .await
            .map_err(Report::new)?;

        let bytes_res = rooms.iter().find_map(|room| {
            if room.starts_at.date() != date {
                return None;
            }

            let res = serialize_using_arena_and_with::<_, RoomRkyv>(room)
                .map_err(RedisError::Serialization);

            Some(res)
        });

        // Serialize and store bytes in a different task so the caller doesn't
        // have to wait
        tokio::spawn(async move {
            let cache = Context::cache();

            for room in rooms {
                let bytes = match serialize_using_arena_and_with::<_, RoomRkyv>(&room) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        warn!(err = ?RedisError::Serialization(err));

                        continue;
                    }
                };

                let key = key_fn(room.starts_at.date());

                if let Err(err) = cache.store_forever(&key, bytes.as_slice()).await {
                    warn!(?err, "Failed to store daily challenge room");
                }
            }
        });

        let bytes = bytes_res.wrap_err_with(|| {
            format!(
                "Missing room for date {}-{:02}-{:02}",
                date.year(),
                date.month() as u8,
                date.day()
            )
        })??;

        CachedArchive::new(bytes).map_err(RedisError::Validation)
    }

    pub async fn osekai_ranking<R>(self) -> RedisResult<Archived<Vec<R::Entry>>>
    where
        R: OsekaiRanking,
        <R as OsekaiRanking>::Entry:
            for<'a> Serialize<SerializerStrategy<'a>, Archived: CheckBytes<ValidatorStrategy<'a>>>,
    {
        const EXPIRE: u64 = 7200;

        let mut key = b"osekai_ranking_".to_vec();
        key.extend_from_slice(R::FORM.as_bytes());

        let mut conn = match Context::cache().fetch(&key).await {
            Ok(Ok(ranking)) => {
                BotMetrics::inc_redis_hit("Osekai ranking");

                return Ok(ranking);
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!(?err, "Failed to fetch osekai ranking");

                None
            }
        };

        let ranking = Context::client().get_osekai_ranking::<R>().await?;

        let bytes = serialize_using_arena(&ranking).map_err(RedisError::Serialization)?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, &key, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store osekai ranking");
            }
        }

        CachedArchive::new(bytes).map_err(RedisError::Validation)
    }

    pub async fn pp_ranking(
        self,
        mode: GameMode,
        page: u32,
        country: Option<&str>,
    ) -> RedisResult<ArchivedRankings> {
        const EXPIRE: u64 = 1800;
        let mut key = format!("pp_ranking_{}_{page}", mode as u8);

        if let Some(country) = country {
            let _ = write!(key, "_{country}");
        }

        let mut conn = match Context::cache().fetch(&key).await {
            Ok(Ok(ranking)) => {
                BotMetrics::inc_redis_hit("PP ranking");

                return Ok(ranking);
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!(?err, "Failed to fetch ranking");

                None
            }
        };

        let mut ranking_fut = Context::osu().performance_rankings(mode).page(page);

        if let Some(country) = country {
            ranking_fut = ranking_fut.country(country);
        }

        let ranking = ranking_fut.await.map_err(Report::new)?;

        let bytes = serialize_using_arena_and_with::<_, RankingsRkyv>(&ranking)
            .map_err(RedisError::Serialization)?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, &key, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store ranking");
            }
        }

        CachedArchive::new(bytes).map_err(RedisError::Validation)
    }

    pub async fn osustats_best(
        self,
        timeframe: OsuStatsBestTimeframe,
        mode: GameMode,
    ) -> Result<OsuStatsBestScores> {
        const EXPIRE: u64 = 3600;
        let key = format!("osustats_best_{}_{}", timeframe as u8, mode as u8);

        let mut conn = match Context::cache()
            .fetch::<_, ArchivedOsuStatsBestScores>(&key)
            .await
        {
            Ok(Ok(scores)) => {
                BotMetrics::inc_redis_hit("osu!stats best");

                return scores.try_deserialize().wrap_err("Failed to deserialize");
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!(?err, "Failed to fetch osustats best");

                None
            }
        };

        let scores = Context::client().get_osustats_best(timeframe, mode).await?;

        if let Some(ref mut conn) = conn {
            match serialize_using_arena(&scores).map_err(RedisError::Serialization) {
                Ok(bytes) => {
                    if let Err(err) = Cache::store(conn, &key, &bytes, EXPIRE).await {
                        warn!(?err, "Failed to store osustats best");
                    }
                }
                Err(err) => warn!(err = ?Report::new(err), "Failed to serialize osustats best"),
            }
        }

        Ok(scores)
    }

    pub async fn osutrack_history(
        self,
        user_id: u32,
        mode: GameMode,
    ) -> RedisResult<ArchivedVec<ArchivedOsuTrackHistoryEntry>> {
        const EXPIRE: u64 = 43_200; // 12 hours
        let key = format!("osutrack_history_{user_id}_{}", mode as u8);

        let mut conn = match Context::cache().fetch(&key).await {
            Ok(Ok(history)) => {
                BotMetrics::inc_redis_hit("osutrack history");

                return Ok(history);
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!(?err, "Failed to fetch osutrack history");

                None
            }
        };

        let history = Context::client()
            .osutrack_user_history(user_id, mode)
            .await?;

        let bytes = serialize_using_arena(&history).map_err(RedisError::Serialization)?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, &key, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store osutrack history");
            }
        }

        CachedArchive::new(bytes).map_err(RedisError::Validation)
    }

    pub async fn snipe_countries(self, mode: GameMode) -> RedisResult<ArchivedSnipeCountries> {
        const EXPIRE: u64 = 43_200; // 12 hours
        let key = format!("snipe_countries_{mode}");

        let mut conn = match Context::cache().fetch(&key).await {
            Ok(Ok(countries)) => {
                BotMetrics::inc_redis_hit("Snipe countries");

                return Ok(countries);
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!(?err, "Failed to fetch snipe countries");

                None
            }
        };

        let countries = Context::client().get_snipe_countries(mode).await?;

        let bytes = serialize_using_arena(&countries).map_err(RedisError::Serialization)?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, &key, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store snipe countries");
            }
        }

        CachedArchive::new(bytes).map_err(RedisError::Validation)
    }

    // Mapset difficulty names for the autocomplete option of the compare command
    pub async fn cs_diffs(
        self,
        command: &InteractionCommand,
        map: &Option<Cow<'_, str>>,
        idx: Option<u32>,
    ) -> Result<Option<CachedArchive<Archived<Vec<MapVersion>>>>, RedisError> {
        const EXPIRE: u64 = 30;

        let idx = match idx {
            Some(idx @ 0..=50) => idx.saturating_sub(1) as usize,
            // Invalid index, ignore
            Some(_) => return Ok(None),
            None => 0,
        };

        let map_ = map.as_deref().unwrap_or_default();
        let key = format!("diffs_{}_{idx}_{map_}", command.id);

        let mut conn = match Context::cache().fetch(&key).await {
            Ok(Ok(diffs)) => {
                BotMetrics::inc_redis_hit("Beatmap difficulties");

                return Ok(Some(diffs));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!(?err, "Failed to fetch diffs");

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

                let score = score_fut.await.wrap_err("Failed to get score")?;

                Some(MapIdType::Map(score.map_id))
            }
            None => {
                let msgs = Context::retrieve_channel_history(command.channel_id)
                    .await
                    .wrap_err("Failed to retrieve channel history")?;

                Context::find_map_id_in_msgs(&msgs, idx).await
            }
        };

        let diffs = match map_id {
            Some(MapIdType::Map(map_id)) => Context::osu_map()
                .versions_by_map(map_id)
                .await
                .map_err(Report::new)?,
            Some(MapIdType::Set(mapset_id)) => Context::osu_map()
                .versions_by_mapset(mapset_id)
                .await
                .map_err(Report::new)?,
            None => Vec::new(),
        };

        let bytes = serialize_using_arena(&diffs).map_err(RedisError::Serialization)?;

        if let Some(ref mut conn) = conn {
            if let Err(err) = Cache::store(conn, &key, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store cs diffs");
            }
        }

        CachedArchive::new(bytes)
            .map(Some)
            .map_err(RedisError::Validation)
    }
}

#[cfg(feature = "twitch")]
const _: () = {
    use bathbot_model::{ArchivedTwitchStream, ArchivedTwitchVideo, rkyv_util::time::UnixEpoch};
    use rkyv::{
        niche::{niched_option::NichedOption, niching::Bool},
        with::NicheInto,
    };

    impl RedisManager {
        pub async fn twitch_stream(
            self,
            user_id: u64,
        ) -> Result<Option<CachedArchive<ArchivedTwitchStream>>, RedisError> {
            const EXPIRE: u64 = 60; // 1 minute
            let key = format!("twitch_stream_{user_id}");

            let mut conn = match Context::cache()
                .fetch::<_, NichedOption<ArchivedTwitchStream, Bool>>(&key)
                .await
            {
                Ok(Ok(stream)) => {
                    BotMetrics::inc_redis_hit("Twitch stream");

                    if stream.is_none() {
                        return Ok(None);
                    }

                    // Re-interpreting the niched option
                    return stream.try_cast().map(Some).map_err(RedisError::Validation);
                }
                Ok(Err(conn)) => Some(conn),
                Err(err) => {
                    warn!(?err, "Failed to fetch twitch stream");

                    None
                }
            };

            let stream = match Context::client().get_twitch_stream(user_id).await {
                Ok(opt) => opt,
                Err(err) => {
                    Context::online_twitch_streams().set_offline_by_user(user_id);

                    return Err(RedisError::Acquire(err));
                }
            };

            if let Some(ref stream) = stream {
                if stream.live {
                    let online_twitch_streams = Context::online_twitch_streams();
                    let guard = online_twitch_streams.guard();
                    online_twitch_streams.set_online(stream, &guard);
                }
            }

            let bytes = serialize_using_arena_and_with::<_, NicheInto<Bool>>(&stream)
                .map_err(RedisError::Serialization)?;

            if let Some(ref mut conn) = conn {
                if let Err(err) = Cache::store(conn, &key, bytes.as_slice(), EXPIRE).await {
                    warn!(?err, "Failed to store twitch stream");
                }
            }

            if stream.is_none() {
                return Ok(None);
            }

            CachedArchive::new(bytes)
                .map(Some)
                .map_err(RedisError::Validation)
        }

        pub async fn last_twitch_vod(
            self,
            user_id: u64,
        ) -> Result<Option<CachedArchive<ArchivedTwitchVideo>>, RedisError> {
            const EXPIRE: u64 = 60; // 1 minute
            let key = format!("twitch_vod_{user_id}");

            let mut conn = match Context::cache()
                .fetch::<_, NichedOption<ArchivedTwitchVideo, UnixEpoch>>(&key)
                .await
            {
                Ok(Ok(vod)) => {
                    BotMetrics::inc_redis_hit("Twitch vod");

                    if vod.is_none() {
                        return Ok(None);
                    }

                    // Re-interpreting the niched option
                    return vod.try_cast().map(Some).map_err(RedisError::Validation);
                }
                Ok(Err(conn)) => Some(conn),
                Err(err) => {
                    warn!(?err, "Failed to fetch twitch vod");

                    None
                }
            };

            let vod = Context::client()
                .get_last_twitch_vod(user_id)
                .await
                .map_err(RedisError::Acquire)?;

            let bytes = serialize_using_arena_and_with::<_, NicheInto<UnixEpoch>>(&vod)
                .map_err(RedisError::Serialization)?;

            if let Some(ref mut conn) = conn {
                if let Err(err) = Cache::store(conn, &key, bytes.as_slice(), EXPIRE).await {
                    warn!(?err, "Failed to store twitch vod");
                }
            }

            if vod.is_none() {
                return Ok(None);
            }

            CachedArchive::new(bytes)
                .map(Some)
                .map_err(RedisError::Validation)
        }
    }
};
