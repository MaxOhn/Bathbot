use std::{collections::HashMap, fmt::Debug, io::Error as IoError, ops::Deref};

use bathbot_client::ClientError;
use bathbot_psql::model::osu::{ArtistTitle, DbBeatmap, DbBeatmapset, DbMapContent, MapVersion};
use bathbot_util::{ExponentialBackoff, IntHasher};
use eyre::{ContextCompat, Report, WrapErr};
use futures::{TryStreamExt, stream::FuturesUnordered};
use rosu_pp::{
    Beatmap,
    any::DifficultyAttributes,
    model::{beatmap::BeatmapAttributesBuilder, mode::GameMode as MapMode},
};
use rosu_v2::prelude::{BeatmapsetExtended, GameMode, OsuError, RankStatus};
use thiserror::Error;
use time::OffsetDateTime;
use tokio::time::sleep;

use super::{PpManager, pp::Mods};
use crate::{
    core::Context,
    util::query::{FilterCriteria, RegularCriteria, Searchable},
};

type Result<T> = eyre::Result<T, MapError>;

#[derive(Copy, Clone)]
pub struct MapManager;

impl MapManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn map(self, map_id: u32, checksum: Option<&str>) -> Result<OsuMap> {
        // Check if map is already stored
        let map_fut = Context::psql().select_osu_map_full(map_id, checksum);

        if let Some((map, mapset, content)) = map_fut.await.wrap_err("Failed to get map")? {
            let (pp_map, map_opt) = self
                .prepare_map(map_id, content)
                .await
                .wrap_err("Failed to prepare map")?;

            match map_opt {
                Some(map) => Ok(OsuMap::new(map, pp_map)),
                None => Ok(OsuMap::new(OsuMapSlim::new(map, mapset), pp_map)),
            }
        } else {
            // Otherwise retrieve mapset and store
            let map_fut = self.retrieve_map(map_id);
            let prepare_fut = self.prepare_map(map_id, DbMapContent::Missing);
            let (map, (pp_map, _)) = tokio::try_join!(map_fut, prepare_fut)?;

            Ok(OsuMap::new(map, pp_map))
        }
    }

    pub async fn pp_map(self, map_id: u32) -> Result<Beatmap> {
        let content = Context::psql()
            .select_beatmap_file_content(map_id)
            .await
            .wrap_err("Failed to get map file content")?
            .map_or(DbMapContent::Missing, DbMapContent::Present);

        let (pp_map, _) = self
            .prepare_map(map_id, content)
            .await
            .wrap_err("Failed to prepare map")?;

        Ok(pp_map)
    }

    /// Returns `Ok(None)` if the map is too suspicious.
    pub async fn difficulty(
        self,
        map_id: u32,
        mode: GameMode,
        mods: impl Into<Mods>,
    ) -> Result<Option<DifficultyAttributes>> {
        async fn inner(
            this: MapManager,
            map_id: u32,
            mode: GameMode,
            mods: Mods,
        ) -> Result<Option<DifficultyAttributes>> {
            let map = this.pp_map(map_id).await.wrap_err("Failed to get pp map")?;

            let attrs = PpManager::from_parsed(&map)
                .mode(mode)
                .mods(mods)
                .difficulty()
                .await
                .cloned();

            Ok(attrs)
        }

        inner(self, map_id, mode, mods.into()).await
    }

    pub async fn map_slim(self, map_id: u32) -> Result<OsuMapSlim> {
        // Check if map is already stored
        let map_fut = Context::psql().select_osu_map_full(map_id, None);

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
        let mut db_maps = Context::psql()
            .select_osu_maps_full(maps_id_checksum)
            .await
            .wrap_err("Failed to get maps")?;

        maps_id_checksum
            .keys()
            .map(|map_id| (*map_id as u32, db_maps.remove(map_id)))
            .map(|(map_id, map_opt)| async move {
                let map = if let Some((map, mapset, filepath)) = map_opt {
                    let (pp_map, map_opt) = self.prepare_map(map_id, filepath).await?;

                    match map_opt {
                        Some(map) => OsuMap::new(map, pp_map),
                        None => OsuMap::new(OsuMapSlim::new(map, mapset), pp_map),
                    }
                } else {
                    let map_fut = self.retrieve_map(map_id);
                    let prepare_fut = self.prepare_map(map_id, DbMapContent::Missing);
                    let (map, (pp_map, _)) = tokio::try_join!(map_fut, prepare_fut)?;

                    OsuMap::new(map, pp_map)
                };

                Ok((map_id, map))
            })
            .collect::<FuturesUnordered<_>>()
            .try_collect()
            .await
    }

    pub async fn artist_title(self, mapset_id: u32) -> Result<ArtistTitle> {
        let artist_title_opt = Context::psql()
            .select_mapset_artist_title(mapset_id)
            .await
            .wrap_err("Failed to get artist title")?;

        if let Some(artist_title) = artist_title_opt {
            return Ok(artist_title);
        }

        let mapset = self.retrieve_mapset(mapset_id).await?;

        Ok(ArtistTitle {
            artist: mapset.artist,
            title: mapset.title,
        })
    }

    fn mapset_to_map_versions(mapset: &BeatmapsetExtended) -> Vec<MapVersion> {
        mapset
            .maps
            .as_ref()
            .map(|maps| {
                maps.iter()
                    .map(|map| MapVersion {
                        map_id: map.map_id as i32,
                        version: map.version.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub async fn versions_by_map(self, map_id: u32) -> Result<Vec<MapVersion>> {
        let versions = Context::psql()
            .select_map_versions_by_map_id(map_id)
            .await
            .wrap_err("Failed to get versions by map")?;

        if !versions.is_empty() {
            return Ok(versions);
        }

        let mapset = match Context::osu().beatmapset_from_map_id(map_id).await {
            Ok(mapset) => mapset,
            Err(OsuError::NotFound) => return Err(MapError::NotFound),
            Err(err) => {
                return Err(MapError::Report(
                    Report::new(err).wrap_err("Failed to retrieve mapset"),
                ));
            }
        };

        let versions = Self::mapset_to_map_versions(&mapset);

        tokio::spawn(async move { self.store(&mapset).await });

        Ok(versions)
    }

    pub async fn versions_by_mapset(self, mapset_id: u32) -> Result<Vec<MapVersion>> {
        let versions = Context::psql()
            .select_map_versions_by_mapset_id(mapset_id)
            .await
            .wrap_err("Failed to get versions by mapset")?;

        if !versions.is_empty() {
            return Ok(versions);
        }

        let mapset = match Context::osu().beatmapset(mapset_id).await {
            Ok(mapset) => mapset,
            Err(OsuError::NotFound) => return Err(MapError::NotFound),
            Err(err) => {
                return Err(MapError::Report(
                    Report::new(err).wrap_err("Failed to retrieve mapset"),
                ));
            }
        };

        let versions = Self::mapset_to_map_versions(&mapset);

        tokio::spawn(async move { self.store(&mapset).await });

        Ok(versions)
    }

    pub async fn store(&self, mapset: &BeatmapsetExtended) {
        if let Err(err) = Context::psql().upsert_beatmapset(mapset).await {
            warn!(?err, "Failed to store mapset");
        }
    }

    /// Request a [`BeatmapsetExtended`] from a map id and turn it into a
    /// [`OsuMapSlim`]
    async fn retrieve_map(self, map_id: u32) -> Result<OsuMapSlim> {
        match Context::osu().beatmapset_from_map_id(map_id).await {
            Ok(mapset) => {
                let mapset_clone = mapset.clone();
                tokio::spawn(async move { self.store(&mapset_clone).await });

                OsuMapSlim::try_from_mapset(mapset, map_id)
            }
            Err(OsuError::NotFound) => Err(MapError::NotFound),
            Err(err) => Err(MapError::Report(
                Report::new(err).wrap_err("Failed to retrieve mapset"),
            )),
        }
    }

    /// Request a [`BeatmapsetExtended`] from a mapset id
    async fn retrieve_mapset(self, mapset_id: u32) -> Result<BeatmapsetExtended> {
        match Context::osu().beatmapset(mapset_id).await {
            Ok(mapset) => {
                let mapset_clone = mapset.clone();
                tokio::spawn(async move { self.store(&mapset_clone).await });

                Ok(mapset)
            }
            Err(OsuError::NotFound) => Err(MapError::NotFound),
            Err(err) => Err(MapError::Report(
                Report::new(err).wrap_err("Failed to retrieve mapset"),
            )),
        }
    }

    /// Make sure the map's current file is available
    async fn prepare_map(
        self,
        map_id: u32,
        content: DbMapContent,
    ) -> Result<(Beatmap, Option<OsuMapSlim>)> {
        match content {
            DbMapContent::Present(content) => match Beatmap::from_bytes(&content) {
                Ok(map) => Ok((map, None)),
                Err(err) => {
                    let wrap = format!("Failed to parse content of map {map_id}");

                    Err(Report::new(err).wrap_err(wrap).into())
                }
            },
            DbMapContent::ChecksumMismatch => {
                info!("Checksum mismatch for map {map_id}, re-downloading...");

                let map_fut = self.download_map_file(map_id);
                let map_slim_fut = self.retrieve_map(map_id);

                let (map_res, map_slim_res) = tokio::join!(map_fut, map_slim_fut);

                let map = map_res.wrap_err("Failed to download map file")?;
                let map_slim = match map_slim_res {
                    Ok(map_slim) => map_slim,
                    Err(err @ MapError::NotFound) => return Err(err),
                    Err(MapError::Report(err)) => {
                        let wrap = "Failed to get map after checksum mismatch";

                        return Err(err.wrap_err(wrap).into());
                    }
                };

                Ok((map, Some(map_slim)))
            }
            DbMapContent::Missing => {
                info!("Missing map {map_id}, downloading...");

                let map = self
                    .download_map_file(map_id)
                    .await
                    .wrap_err("Failed to download map file")?;

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
            NoContent,
            Ratelimited,
            DecodeFail(
                // false positive; used when logging
                #[allow(unused)] IoError,
            ),
        }

        for (duration, i) in backoff.take(ATTEMPTS).zip(2..) {
            let bytes_fut = Context::client().get_map_file(map_id);

            let reason = match bytes_fut.await {
                Ok(bytes) if !bytes.is_empty() => match Beatmap::from_bytes(&bytes) {
                    Ok(map) => {
                        let db_fut = Context::psql().insert_beatmap_file_content(map_id, &bytes);

                        if let Err(err) = db_fut.await {
                            warn!(map_id, ?err, "Failed to insert file content");
                        } else {
                            info!("Downloaded {map_id}.osu successfully");
                        }

                        return Ok(map);
                    }
                    Err(err) => BackoffReason::DecodeFail(err),
                },
                Ok(_) => BackoffReason::NoContent,
                Err(ClientError::Ratelimited) => BackoffReason::Ratelimited,
                Err(err) => {
                    let err = Report::new(err).wrap_err("Failed to request map file");

                    return Err(err.into());
                }
            };

            warn!(
                ?reason,
                "Failed map download; backoff {duration:?} and then retry attempt #{i}"
            );

            sleep(duration).await;
        }

        let err = eyre!("Reached retry limit and still failed to download {map_id}.osu");

        Err(MapError::Report(err))
    }
}

#[derive(Clone)]
pub struct OsuMapSlim {
    map: DbBeatmap,
    mapset: DbBeatmapset,
}

impl OsuMapSlim {
    fn new(map: DbBeatmap, mapset: DbBeatmapset) -> Self {
        Self { map, mapset }
    }

    fn try_from_mapset(mut mapset: BeatmapsetExtended, map_id: u32) -> Result<Self> {
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

    pub fn n_sliders(&self) -> usize {
        self.map.count_sliders as usize
    }

    pub fn n_spinners(&self) -> usize {
        self.map.count_spinners as usize
    }

    pub fn n_objects(&self) -> u32 {
        (self.map.count_circles + self.map.count_sliders + self.map.count_spinners) as u32
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

impl Searchable<RegularCriteria<'_>> for OsuMapSlim {
    #[inline]
    fn matches(&self, criteria: &FilterCriteria<RegularCriteria<'_>>) -> bool {
        self.map.matches(criteria) && self.mapset.matches(criteria)
    }
}

#[derive(Clone)]
pub struct OsuMap {
    map: OsuMapSlim,
    pub pp_map: Beatmap,
}

impl OsuMap {
    fn new(map: OsuMapSlim, pp_map: Beatmap) -> Self {
        Self { map, pp_map }
    }

    pub fn mode(&self) -> GameMode {
        (self.pp_map.mode as u8).into()
    }

    pub fn attributes(&self) -> BeatmapAttributesBuilder {
        self.pp_map.attributes()
    }

    pub fn convert_mut(&mut self, mode: GameMode) {
        let mode = match mode {
            GameMode::Osu => MapMode::Osu,
            GameMode::Taiko => MapMode::Taiko,
            GameMode::Catch => MapMode::Catch,
            GameMode::Mania => MapMode::Mania,
        };

        // FIXME: use mods
        let _ = self.pp_map.convert_mut(mode, &Default::default());
    }

    pub fn convert(mut self, mode: GameMode) -> Self {
        self.convert_mut(mode);

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

impl Searchable<RegularCriteria<'_>> for OsuMap {
    #[inline]
    fn matches(&self, criteria: &FilterCriteria<RegularCriteria<'_>>) -> bool {
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
