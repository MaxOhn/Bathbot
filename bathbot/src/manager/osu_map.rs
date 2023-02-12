use std::{borrow::Cow, collections::HashMap, fmt::Debug, ops::Deref};

use bathbot_client::ClientError;
use bathbot_psql::{
    model::osu::{ArtistTitle, DbBeatmap, DbBeatmapset, DbMapPath, MapVersion},
    Database,
};
use bathbot_util::{ExponentialBackoff, IntHasher};
use eyre::{ContextCompat, Report, WrapErr};
use rosu_pp::{Beatmap, DifficultyAttributes, GameMode as Mode, ParseError};
use rosu_v2::prelude::{Beatmapset, GameMode, GameMods, OsuError, RankStatus};
use thiserror::Error;
use time::OffsetDateTime;
use tokio::{fs, time::sleep};

use crate::{
    core::{BotConfig, Context},
    util::query::{FilterCriteria, Searchable},
};

use super::PpManager;

type Result<T> = eyre::Result<T, MapError>;

#[derive(Copy, Clone)]
pub struct MapManager<'d> {
    psql: &'d Database,
    ctx: &'d Context,
}

impl<'d> MapManager<'d> {
    pub fn new(psql: &'d Database, ctx: &'d Context) -> Self {
        Self { psql, ctx }
    }

    pub async fn map(self, map_id: u32, checksum: Option<&str>) -> Result<OsuMap> {
        // Check if map is already stored
        let map_fut = self.psql.select_osu_map_full(map_id, checksum);

        if let Some((map, mapset, filepath)) = map_fut.await.wrap_err("Failed to get map")? {
            let (pp_map, map_opt) = self
                .prepare_map(map_id, filepath)
                .await
                .wrap_err("Failed to prepare map")?;

            match map_opt {
                Some(map) => Ok(OsuMap::new(map, pp_map)),
                None => Ok(OsuMap::new(OsuMapSlim::new(map, mapset), pp_map)),
            }
        } else {
            // Otherwise retrieve mapset and store
            let map_fut = self.retrieve_map(map_id);
            let prepare_fut = self.prepare_map(map_id, DbMapPath::Missing);
            let (map, (pp_map, _)) = tokio::try_join!(map_fut, prepare_fut)?;

            Ok(OsuMap::new(map, pp_map))
        }
    }

    pub async fn pp_map(self, map_id: u32) -> Result<Beatmap> {
        let filepath = self
            .psql
            .select_beatmap_file(map_id)
            .await
            .wrap_err("Failed to get filepath")?
            .map_or(DbMapPath::Missing, DbMapPath::Present);

        let (pp_map, _) = self
            .prepare_map(map_id, filepath)
            .await
            .wrap_err("Failed to prepare map")?;

        Ok(pp_map)
    }

    pub async fn difficulty(
        self,
        map_id: u32,
        mode: GameMode,
        mods: GameMods,
    ) -> Result<DifficultyAttributes> {
        let attrs_fut = self
            .psql
            .select_map_difficulty_attrs(map_id, mode, mods.bits());

        if let Some(attrs) = attrs_fut.await.wrap_err("Failed to get attributes")? {
            return Ok(attrs);
        }

        let map = self.pp_map(map_id).await.wrap_err("Failed to get pp map")?;

        let attrs = PpManager::from_parsed(&map, map_id, mode, false, self.psql)
            .mods(mods)
            .difficulty()
            .await
            .to_owned();

        Ok(attrs)
    }

    pub async fn map_slim(self, map_id: u32) -> Result<OsuMapSlim> {
        // Check if map is already stored
        let map_fut = self.psql.select_osu_map_full(map_id, None);

        if let Some((map, mapset, _)) = map_fut.await.wrap_err("Failed to get map")? {
            Ok(OsuMapSlim::new(map, mapset))
        } else {
            // Otherwise retrieve mapset and store
            self.retrieve_map(map_id).await
        }
    }

    pub async fn maps(
        self,
        maps_id_checksum: &HashMap<i32, Option<&str>, IntHasher>,
    ) -> Result<HashMap<u32, OsuMap, IntHasher>> {
        let mut db_maps = self
            .psql
            .select_osu_maps_full(maps_id_checksum)
            .await
            .wrap_err("failed to get maps")?;

        let iter = maps_id_checksum
            .keys()
            .map(|map_id| (*map_id as u32, db_maps.remove(map_id)));

        let mut maps = HashMap::with_capacity_and_hasher(maps_id_checksum.len(), IntHasher);

        // Having this non-async is pretty sad but the many I/O operations appear
        // to cause thread-limitation issues when collected into a FuturesUnordered.
        for (map_id, map_opt) in iter {
            let map = if let Some((map, mapset, filepath)) = map_opt {
                let (pp_map, map_opt) = self.prepare_map(map_id, filepath).await?;

                match map_opt {
                    Some(map) => OsuMap::new(map, pp_map),
                    None => OsuMap::new(OsuMapSlim::new(map, mapset), pp_map),
                }
            } else {
                let map_fut = self.retrieve_map(map_id);
                let prepare_fut = self.prepare_map(map_id, DbMapPath::Missing);
                let (map, (pp_map, _)) = tokio::try_join!(map_fut, prepare_fut)?;

                OsuMap::new(map, pp_map)
            };

            maps.insert(map_id, map);
        }

        Ok(maps)
    }

    pub async fn artist_title(self, mapset_id: u32) -> Result<ArtistTitle> {
        let artist_title_opt = self
            .psql
            .select_mapset_artist_title(mapset_id)
            .await
            .wrap_err("failed to get artist title")?;

        if let Some(artist_title) = artist_title_opt {
            return Ok(artist_title);
        }

        let mapset = self.retrieve_mapset(mapset_id).await?;

        Ok(ArtistTitle {
            artist: mapset.artist,
            title: mapset.title,
        })
    }

    pub async fn mapset(self, mapset_id: u32) -> Result<DbBeatmapset> {
        let mapset_fut = self.psql.select_mapset(mapset_id);

        if let Some(mapset) = mapset_fut.await.wrap_err("failed to get mapset")? {
            Ok(mapset)
        } else {
            let mapset = self.retrieve_mapset(mapset_id).await?;

            let mapset = DbBeatmapset {
                mapset_id: mapset.mapset_id as i32,
                user_id: mapset.creator_id as i32,
                artist: mapset.artist,
                title: mapset.title,
                creator: mapset.creator_name.into_string(),
                rank_status: mapset.status as i16,
                ranked_date: mapset.ranked_date,
                thumbnail: mapset.covers.list,
                cover: mapset.covers.cover,
            };

            Ok(mapset)
        }
    }

    pub async fn versions_by_map(self, map_id: u32) -> Result<Vec<MapVersion>> {
        let versions = self
            .psql
            .select_map_versions_by_map_id(map_id)
            .await
            .wrap_err("failed to get versions by map")?;

        if !versions.is_empty() {
            return Ok(versions);
        }

        match self.ctx.osu().beatmapset_from_map_id(map_id).await {
            Ok(mapset) => {
                if let Err(err) = self.store(&mapset).await {
                    warn!("{err:?}");
                }
            }
            Err(OsuError::NotFound) => return Err(MapError::NotFound),
            Err(err) => {
                return Err(MapError::Report(
                    Report::new(err).wrap_err("failed to retrieve mapset"),
                ))
            }
        }

        self.psql
            .select_map_versions_by_map_id(map_id)
            .await
            .wrap_err("failed to get versions by map")
            .map_err(MapError::Report)
    }

    pub async fn versions_by_mapset(self, mapset_id: u32) -> Result<Vec<MapVersion>> {
        let versions = self
            .psql
            .select_map_versions_by_mapset_id(mapset_id)
            .await
            .wrap_err("failed to get versions by mapset")
            .map_err(MapError::Report)?;

        if !versions.is_empty() {
            return Ok(versions);
        }

        match self.ctx.osu().beatmapset(mapset_id).await {
            Ok(mapset) => {
                if let Err(err) = self.store(&mapset).await {
                    warn!("{err:?}");
                }
            }
            Err(OsuError::NotFound) => return Err(MapError::NotFound),
            Err(err) => {
                return Err(MapError::Report(
                    Report::new(err).wrap_err("failed to retrieve mapset"),
                ))
            }
        }

        self.psql
            .select_map_versions_by_mapset_id(mapset_id)
            .await
            .wrap_err("failed to get versions by mapset")
            .map_err(MapError::Report)
    }

    pub async fn store(self, mapset: &Beatmapset) -> eyre::Result<()> {
        self.psql
            .upsert_beatmapset(mapset)
            .await
            .wrap_err("failed to store mapset")
    }

    /// Request a [`Beatmapset`] from a map id and turn it into a [`OsuMapSlim`]
    async fn retrieve_map(&self, map_id: u32) -> Result<OsuMapSlim> {
        match self.ctx.osu().beatmapset_from_map_id(map_id).await {
            Ok(mapset) => {
                if let Err(err) = self.store(&mapset).await {
                    warn!("{err:?}");
                }

                OsuMapSlim::try_from_mapset(mapset, map_id)
            }
            Err(OsuError::NotFound) => Err(MapError::NotFound),
            Err(err) => Err(MapError::Report(
                Report::new(err).wrap_err("failed to retrieve mapset"),
            )),
        }
    }

    /// Request a [`Beatmapset`] from a mapset id
    async fn retrieve_mapset(&self, mapset_id: u32) -> Result<Beatmapset> {
        match self.ctx.osu().beatmapset(mapset_id).await {
            Ok(mapset) => {
                if let Err(err) = self.store(&mapset).await {
                    warn!("{err:?}");
                }

                Ok(mapset)
            }
            Err(OsuError::NotFound) => Err(MapError::NotFound),
            Err(err) => Err(MapError::Report(
                Report::new(err).wrap_err("failed to retrieve mapset"),
            )),
        }
    }

    /// Make sure the map's current file is available
    async fn prepare_map(
        &self,
        map_id: u32,
        filepath: DbMapPath,
    ) -> Result<(Beatmap, Option<OsuMapSlim>)> {
        match filepath {
            DbMapPath::Present(path) => match Beatmap::from_path(&path).await {
                Ok(map) => Ok((map, None)),
                Err(err) => {
                    if let Err(err) = fs::remove_file(&path).await {
                        let wrap = format!("failed to delete file {path}");
                        warn!("{:?}", Report::new(err).wrap_err(wrap));
                    }

                    let wrap = format!("failed to parse map `{path}`");

                    Err(Report::new(err).wrap_err(wrap).into())
                }
            },
            DbMapPath::ChecksumMismatch => {
                info!("Checksum mismatch for map {map_id}, re-downloading...");

                let map_fut = self.download_map_file(map_id);
                let map_slim_fut = self.retrieve_map(map_id);

                let (map_res, map_slim_res) = tokio::join!(map_fut, map_slim_fut);

                let map = map_res.wrap_err("failed to download map file")?;
                let map_slim = match map_slim_res {
                    Ok(map_slim) => map_slim,
                    Err(err @ MapError::NotFound) => return Err(err),
                    Err(MapError::Report(report)) => {
                        let wrap = "failed to get map after checksum mismatch";

                        return Err(report.wrap_err(wrap).into());
                    }
                };

                Ok((map, Some(map_slim)))
            }
            DbMapPath::Missing => {
                info!("Missing map {map_id}, downloading...");

                let map = self
                    .download_map_file(map_id)
                    .await
                    .wrap_err("failed to download map file")?;

                Ok((map, None))
            }
        }
    }

    /// Download a map's file and retry if it failed
    async fn download_map_file(&self, map_id: u32) -> Result<Beatmap> {
        let backoff = ExponentialBackoff::new(2).factor(500).max_delay(10_000);
        const ATTEMPTS: usize = 10;

        #[derive(Debug)]
        enum BackoffReason {
            Ratelimited,
            ParseFail(ParseError),
        }

        for (duration, i) in backoff.take(ATTEMPTS).zip(2..) {
            let bytes_fut = self.ctx.client().get_map_file(map_id);

            let reason = match bytes_fut.await {
                Ok(bytes) => match Beatmap::parse(bytes.as_ref()).await {
                    Ok(map) => {
                        let mut map_path = BotConfig::get().paths.maps.clone();
                        map_path.push(format!("{map_id}.osu"));
                        let map_path_str = map_path.to_string_lossy();

                        let write_fut = fs::write(&map_path, &bytes);
                        let db_fut = self.psql.insert_beatmap_file(map_id, &map_path_str);

                        let (write_res, db_res) = tokio::join!(write_fut, db_fut);
                        write_res.wrap_err("failed writing to file")?;

                        if let Err(err) = db_res {
                            warn!("{:?}", err.wrap_err("failed to insert map file"));
                        }

                        info!("Downloaded {map_id}.osu successfully");

                        return Ok(map);
                    }
                    Err(err) => BackoffReason::ParseFail(err),
                },
                Err(ClientError::Ratelimited) => BackoffReason::Ratelimited,
                Err(err) => {
                    let err = Report::new(err).wrap_err("failed to request map file");

                    return Err(err.into());
                }
            };

            warn!(
                "Failed map download because `{reason:?}`; \
                backoff {duration:?} and then retry attempt #{i}"
            );

            sleep(duration).await;
        }

        let err = eyre!("reached retry limit and still failed to download {map_id}.osu");

        Err(MapError::Report(err))
    }
}

pub struct OsuMapSlim {
    map: DbBeatmap,
    mapset: DbBeatmapset,
}

impl OsuMapSlim {
    fn new(map: DbBeatmap, mapset: DbBeatmapset) -> Self {
        Self { map, mapset }
    }

    fn try_from_mapset(mut mapset: Beatmapset, map_id: u32) -> Result<Self> {
        let map = mapset
            .maps
            .take()
            .and_then(|maps| maps.into_iter().find(|map| map.map_id == map_id))
            .wrap_err("missing map in mapset")?;

        let mapset = DbBeatmapset {
            mapset_id: mapset.mapset_id as i32,
            user_id: mapset.creator_id as i32,
            artist: mapset.artist,
            title: mapset.title,
            creator: mapset.creator_name.into_string(),
            rank_status: mapset.status as i16,
            ranked_date: mapset.ranked_date,
            thumbnail: mapset.covers.list,
            cover: mapset.covers.cover,
        };

        let map = DbBeatmap {
            map_id: map.map_id as i32,
            mapset_id: map.mapset_id as i32,
            user_id: map.creator_id as i32,
            map_version: map.version,
            seconds_drain: map.seconds_drain as i32,
            count_circles: map.count_circles as i32,
            count_sliders: map.count_sliders as i32,
            count_spinners: map.count_spinners as i32,
            bpm: map.bpm,
        };

        Ok(Self::new(map, mapset))
    }

    pub fn map_id(&self) -> u32 {
        self.map.map_id as u32
    }

    pub fn mapset_id(&self) -> u32 {
        self.map.mapset_id as u32
    }

    pub fn creator_id(&self) -> u32 {
        self.map.user_id as u32
    }

    pub fn version(&self) -> &str {
        self.map.map_version.as_str()
    }

    pub fn artist(&self) -> &str {
        self.mapset.artist.as_str()
    }

    pub fn title(&self) -> &str {
        self.mapset.title.as_str()
    }

    pub fn creator(&self) -> &str {
        self.mapset.creator.as_str()
    }

    pub fn seconds_drain(&self) -> u32 {
        self.map.seconds_drain as u32
    }

    pub fn bpm(&self) -> f32 {
        self.map.bpm
    }

    pub fn n_circles(&self) -> usize {
        self.map.count_circles as usize
    }

    pub fn n_objects(&self) -> usize {
        (self.map.count_circles + self.map.count_sliders + self.map.count_spinners) as usize
    }

    pub fn status(&self) -> RankStatus {
        let status = self.mapset.rank_status as i8;

        status.try_into().unwrap_or_else(|_| {
            panic!(
                "cannot convert status `{status}` of map {map} into RankStatus",
                status = self.mapset.rank_status,
                map = self.map.map_id,
            )
        })
    }

    pub fn ranked_date(&self) -> Option<OffsetDateTime> {
        self.mapset.ranked_date
    }

    pub fn thumbnail(&self) -> &str {
        self.mapset.thumbnail.as_str()
    }

    pub fn cover(&self) -> &str {
        self.mapset.cover.as_str()
    }
}

impl Searchable for OsuMapSlim {
    #[inline]
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
        self.map.matches(criteria) && self.mapset.matches(criteria)
    }
}

pub struct OsuMap {
    map: OsuMapSlim,
    pub pp_map: Beatmap,
    pub is_convert: bool,
}

impl OsuMap {
    fn new(map: OsuMapSlim, pp_map: Beatmap) -> Self {
        Self {
            map,
            pp_map,
            is_convert: false,
        }
    }

    pub fn mode(&self) -> GameMode {
        GameMode::from(self.pp_map.mode as u8)
    }

    pub fn ar(&self) -> f32 {
        self.pp_map.ar
    }

    pub fn cs(&self) -> f32 {
        self.pp_map.cs
    }

    pub fn hp(&self) -> f32 {
        self.pp_map.hp
    }

    pub fn od(&self) -> f32 {
        self.pp_map.od
    }

    pub fn convert(mut self, mode: GameMode) -> Self {
        let mode = match mode {
            GameMode::Osu => Mode::Osu,
            GameMode::Taiko => Mode::Taiko,
            GameMode::Catch => Mode::Catch,
            GameMode::Mania => Mode::Mania,
        };

        if let Cow::Owned(map) = self.pp_map.convert_mode(mode) {
            self.pp_map = map;
            self.is_convert = true;
        } else if mode == Mode::Catch && self.pp_map.mode != Mode::Catch {
            self.pp_map.mode = mode;
            self.is_convert = true;
        }

        self
    }
}

impl Deref for OsuMap {
    type Target = OsuMapSlim;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl Searchable for OsuMap {
    #[inline]
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
        self.map.matches(criteria) && self.pp_map.matches(criteria)
    }
}

#[derive(Debug, Error)]
pub enum MapError {
    #[error("map(set) not found")]
    NotFound,
    #[error(transparent)]
    Report(#[from] Report),
}
