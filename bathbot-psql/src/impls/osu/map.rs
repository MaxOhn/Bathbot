use std::{collections::HashMap, hash::BuildHasher};

use eyre::{Result, WrapErr};
use futures::StreamExt;
use rosu_v2::prelude::BeatmapExtended;
use sqlx::{Postgres, Transaction};

use crate::{
    model::osu::{DbBeatmap, DbBeatmapset, DbMapFilename, MapVersion},
    Database,
};

impl Database {
    pub async fn select_osu_map_full(
        &self,
        map_id: u32,
        checksum: Option<&str>,
    ) -> Result<Option<(DbBeatmap, DbBeatmapset, DbMapFilename)>> {
        let query = sqlx::query!(
            r#"
SELECT 
  map.map_id, 
  map.mapset_id, 
  map.user_id, 
  map.checksum, 
  map.map_version, 
  map.seconds_drain, 
  map.count_circles, 
  map.count_sliders, 
  map.count_spinners, 
  map.bpm, 
  mapset.artist, 
  mapset.title, 
  mapset.creator, 
  mapset.rank_status, 
  mapset.ranked_date, 
  mapset.thumbnail, 
  mapset.cover, 
  (
    SELECT 
      map_filepath 
    FROM 
      osu_map_files 
    WHERE 
      map_id = $1
  ) 
FROM 
  (
    SELECT 
      * 
    FROM 
      osu_maps 
    WHERE 
      map_id = $1
  ) AS map 
  JOIN (
    SELECT 
      mapset_id, 
      artist, 
      title, 
      creator, 
      rank_status, 
      ranked_date, 
      thumbnail, 
      cover 
    FROM 
      osu_mapsets
  ) AS mapset ON map.mapset_id = mapset.mapset_id"#,
            map_id as i32
        );

        let row_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?;

        let Some(row) = row_opt else { return Ok(None) };

        let map = DbBeatmap {
            map_id: row.map_id,
            mapset_id: row.mapset_id,
            user_id: row.user_id,
            map_version: row.map_version,
            seconds_drain: row.seconds_drain,
            count_circles: row.count_circles,
            count_sliders: row.count_sliders,
            count_spinners: row.count_spinners,
            bpm: row.bpm,
        };

        let mapset = DbBeatmapset {
            mapset_id: row.mapset_id,
            user_id: row.user_id,
            artist: row.artist,
            title: row.title,
            creator: row.creator,
            rank_status: row.rank_status,
            ranked_date: row.ranked_date,
            thumbnail: row.thumbnail,
            cover: row.cover,
        };

        let filepath = match (row.map_filepath, checksum) {
            (Some(path), Some(checksum)) if row.checksum == checksum => {
                DbMapFilename::Present(path.into_boxed_str())
            }
            (Some(path), None) => DbMapFilename::Present(path.into_boxed_str()),
            (Some(_), Some(_)) => DbMapFilename::ChecksumMismatch,
            (None, _) => DbMapFilename::Missing,
        };

        Ok(Some((map, mapset, filepath)))
    }

    pub async fn select_osu_maps_full<S>(
        &self,
        maps_id_checksum: &HashMap<i32, Option<&str>, S>,
    ) -> Result<HashMap<i32, (DbBeatmap, DbBeatmapset, DbMapFilename), S>>
    where
        S: Default + BuildHasher,
    {
        let map_ids: Vec<_> = maps_id_checksum.keys().copied().collect();

        let query = sqlx::query!(
            r#"
SELECT 
  map.map_id, 
  map.mapset_id, 
  map.user_id, 
  map.checksum, 
  map.map_version, 
  map.seconds_drain, 
  map.count_circles, 
  map.count_sliders, 
  map.count_spinners, 
  map.bpm, 
  mapset.artist, 
  mapset.title, 
  mapset.creator, 
  mapset.rank_status, 
  mapset.ranked_date, 
  mapset.thumbnail, 
  mapset.cover, 
  COALESCE(files.map_filepath) AS map_filepath 
FROM 
  (
    SELECT 
      * 
    FROM 
      osu_maps 
    WHERE 
      map_id = ANY($1)
  ) AS map 
  JOIN (
    SELECT 
      mapset_id, 
      artist, 
      title, 
      creator, 
      rank_status, 
      ranked_date, 
      thumbnail, 
      cover 
    FROM 
      osu_mapsets
  ) AS mapset ON map.mapset_id = mapset.mapset_id 
  LEFT JOIN (
    SELECT 
      map_id, 
      map_filepath 
    FROM 
      osu_map_files
  ) AS files ON map.map_id = files.map_id"#,
            &map_ids
        );

        let mut rows = query.fetch(self);
        let mut maps = HashMap::with_capacity_and_hasher(map_ids.len(), S::default());

        while let Some(row_res) = rows.next().await {
            let row = row_res.wrap_err("failed to fetch next")?;

            let map = DbBeatmap {
                map_id: row.map_id,
                mapset_id: row.mapset_id,
                user_id: row.user_id,
                map_version: row.map_version,
                seconds_drain: row.seconds_drain,
                count_circles: row.count_circles,
                count_sliders: row.count_sliders,
                count_spinners: row.count_spinners,
                bpm: row.bpm,
            };

            let mapset = DbBeatmapset {
                mapset_id: row.mapset_id,
                user_id: row.user_id,
                artist: row.artist,
                title: row.title,
                creator: row.creator,
                rank_status: row.rank_status,
                ranked_date: row.ranked_date,
                thumbnail: row.thumbnail,
                cover: row.cover,
            };

            let checksum = maps_id_checksum.get(&map.map_id).and_then(Option::as_ref);

            let filepath = match (row.map_filepath, checksum) {
                (Some(path), Some(&checksum)) if row.checksum == checksum => {
                    DbMapFilename::Present(path.into_boxed_str())
                }
                (Some(path), None) => DbMapFilename::Present(path.into_boxed_str()),
                (Some(_), Some(_)) => DbMapFilename::ChecksumMismatch,
                (None, _) => DbMapFilename::Missing,
            };

            maps.insert(map.map_id, (map, mapset, filepath));
        }

        // TODO: remove after fixing https://github.com/MaxOhn/Bathbot/issues/849
        {
            struct DebugMaps<'a, S> {
                maps: &'a HashMap<i32, (DbBeatmap, DbBeatmapset, DbMapFilename), S>,
                checksums: &'a HashMap<i32, Option<&'a str>, S>,
            }

            impl<'a, S> DebugMaps<'a, S> {
                fn new(
                    maps: &'a HashMap<i32, (DbBeatmap, DbBeatmapset, DbMapFilename), S>,
                    checksums: &'a HashMap<i32, Option<&'a str>, S>,
                ) -> Self {
                    Self { maps, checksums }
                }
            }

            impl<S: std::hash::BuildHasher> std::fmt::Display for DebugMaps<'_, S> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let iter = self.maps.iter().map(|(map_id, (.., filename))| {
                        let filename = match filename {
                            DbMapFilename::Present(_) => "Present",
                            DbMapFilename::ChecksumMismatch => "ChecksumMismatch",
                            DbMapFilename::Missing => "Missing",
                        };

                        let checksum = self
                            .checksums
                            .get(map_id)
                            .and_then(Option::as_ref)
                            .copied()
                            .unwrap_or("None");

                        (*map_id, (filename, checksum))
                    });

                    f.debug_map().entries(iter).finish()
                }
            }

            debug!(checksums = %DebugMaps::new(&maps, maps_id_checksum), "Found maps");
        }

        Ok(maps)
    }

    pub async fn select_beatmap_file(&self, map_id: u32) -> Result<Option<Box<str>>> {
        let query = sqlx::query!(
            r#"
SELECT 
  map_filepath 
FROM 
  osu_map_files 
WHERE 
  map_id = $1"#,
            map_id as i32
        );

        query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")
            .map(|row_opt| row_opt.map(|row| row.map_filepath.into_boxed_str()))
    }

    pub async fn select_map_versions_by_map_id(&self, map_id: u32) -> Result<Vec<MapVersion>> {
        let query = sqlx::query_as!(
            MapVersion,
            r#"
SELECT 
  map_id, 
  map_version AS version
FROM 
  (
    SELECT 
      map_id, 
      mapset_id, 
      map_version 
    FROM 
      osu_maps
  ) AS maps 
  JOIN (
    SELECT 
      mapset_id 
    FROM 
      osu_maps 
    WHERE 
      map_id = $1
  ) AS mapset ON maps.mapset_id = mapset.mapset_id"#,
            map_id as i32
        );

        query.fetch_all(self).await.wrap_err("failed to fetch all")
    }

    pub async fn select_map_versions_by_mapset_id(
        &self,
        mapset_id: u32,
    ) -> Result<Vec<MapVersion>> {
        let query = sqlx::query_as!(
            MapVersion,
            r#"
SELECT 
  DISTINCT ON (version) map_id, 
  map_version AS version 
FROM 
  osu_maps 
WHERE 
  mapset_id = $1 
ORDER BY 
  version, 
  last_update DESC"#,
            mapset_id as i32,
        );

        query.fetch_all(self).await.wrap_err("failed to fetch all")
    }

    pub async fn insert_beatmap_file(&self, map_id: u32, path: impl AsRef<str>) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO osu_map_files (map_id, map_filepath) 
VALUES 
  ($1, $2) ON CONFLICT (map_id) DO 
UPDATE 
SET 
  map_filepath = $2"#,
            map_id as i32,
            path.as_ref()
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }

    pub(super) async fn delete_beatmaps_of_beatmapset(
        tx: &mut Transaction<'_, Postgres>,
        mapset_id: u32,
    ) -> Result<()> {
        let query = sqlx::query!(
            r#"DELETE FROM osu_maps WHERE mapset_id = $1"#,
            mapset_id as i32
        );

        query
            .execute(&mut **tx)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }

    pub(super) async fn upsert_beatmap(
        tx: &mut Transaction<'_, Postgres>,
        map: &BeatmapExtended,
    ) -> Result<()> {
        let checksum = match map.checksum {
            Some(ref checksum) => checksum,
            None => {
                warn!("Beatmap must contain checksum to be inserted into DB");

                return Ok(());
            }
        };

        // https://stackoverflow.com/questions/39058213/differentiate-inserted-and-updated-rows-in-upsert-using-system-columns
        let query = sqlx::query!(
            r#"
INSERT INTO osu_maps (
  map_id, mapset_id, user_id, checksum, 
  map_version, seconds_total, seconds_drain, 
  count_circles, count_sliders, count_spinners, 
  hp, cs, od, ar, bpm, gamemode
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11, $12, $13, $14, $15, $16
  ) ON CONFLICT (map_id) DO 
UPDATE 
SET 
  mapset_id = $2, 
  user_id = $3, 
  checksum = $4, 
  map_version = $5, 
  seconds_total = $6, 
  seconds_drain = $7, 
  count_circles = $8, 
  count_sliders = $9, 
  count_spinners = $10, 
  hp = $11, 
  cs = $12, 
  od = $13, 
  ar = $14, 
  bpm = $15, 
  gamemode = $16, 
  last_update = NOW() 
WHERE 
  osu_maps.checksum IS DISTINCT 
FROM 
  EXCLUDED.checksum RETURNING (xmax = 0) AS inserted"#,
            map.map_id as i32,
            map.mapset_id as i32,
            map.creator_id as i32,
            checksum,
            map.version,
            map.seconds_total as i32,
            map.seconds_drain as i32,
            map.count_circles as i32,
            map.count_sliders as i32,
            map.count_spinners as i32,
            map.hp,
            map.cs,
            map.od,
            map.ar,
            map.bpm,
            map.mode as i16,
        );

        // `None` if both map_id and checksum were the same i.e. no change
        let row_opt = query
            .fetch_optional(&mut **tx)
            .await
            .wrap_err("Failed to fetch optional")?;

        let updated = row_opt
            .and_then(|row| row.inserted)
            .map_or(false, |inserted| !inserted);

        // Either new entry or different checksum
        // => map has changed so we should delete all maps of its mapset
        if updated {
            let query = sqlx::query!(
                r#"
DELETE FROM 
  osu_map_files 
WHERE 
  map_id = $1"#,
                map.map_id as i32,
            );

            query
                .execute(&mut **tx)
                .await
                .wrap_err("Failed to delete from osu_map_files")?;
        }

        Ok(())
    }
}
