use std::{collections::HashMap, hash::BuildHasher};

use eyre::{Result, WrapErr};
use futures::StreamExt;
use rosu_v2::prelude::BeatmapExtended;
use sqlx::{Postgres, Transaction};

use crate::{
    Database,
    model::osu::{DbBeatmap, DbBeatmapset, DbMapContent, MapVersion},
};

impl Database {
    pub async fn select_osu_map_full(
        &self,
        map_id: u32,
        checksum: Option<&str>,
    ) -> Result<Option<(DbBeatmap, DbBeatmapset, DbMapContent)>> {
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
      content 
    FROM 
      osu_map_file_content 
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

        let content = match (row.content, checksum) {
            (Some(content), Some(checksum)) if row.checksum == checksum => {
                DbMapContent::Present(content)
            }
            (Some(content), None) => DbMapContent::Present(content),
            (Some(_), Some(_)) => DbMapContent::ChecksumMismatch,
            (None, _) => DbMapContent::Missing,
        };

        Ok(Some((map, mapset, content)))
    }

    pub async fn select_osu_maps_full<S>(
        &self,
        maps_id_checksum: &HashMap<i32, Option<&str>, S>,
    ) -> Result<HashMap<i32, (DbBeatmap, DbBeatmapset, DbMapContent), S>>
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
  COALESCE(files_content.content) AS content 
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
      content 
    FROM 
      osu_map_file_content
  ) AS files_content ON map.map_id = files_content.map_id"#,
            &map_ids
        );

        let mut rows = query.fetch(self);
        let mut maps = HashMap::with_capacity_and_hasher(map_ids.len(), S::default());

        while let Some(row_res) = rows.next().await {
            let row = row_res.wrap_err("Failed to fetch next")?;

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

            let content = match (row.content, checksum) {
                (Some(content), Some(&checksum)) if row.checksum == checksum => {
                    DbMapContent::Present(content)
                }
                (Some(content), None) => DbMapContent::Present(content),
                (Some(_), Some(_)) => DbMapContent::ChecksumMismatch,
                (None, _) => DbMapContent::Missing,
            };

            maps.insert(map.map_id, (map, mapset, content));
        }

        Ok(maps)
    }

    pub async fn select_beatmap_file_content(&self, map_id: u32) -> Result<Option<Vec<u8>>> {
        let query = sqlx::query!(
            r#"
  SELECT
    content
  FROM
    osu_map_file_content
  WHERE
    map_id = $1"#,
            map_id as i32
        );

        query
            .fetch_optional(self)
            .await
            .wrap_err("Failed to fetch optional")
            .map(|row_opt| row_opt.map(|row| row.content))
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

    pub async fn insert_beatmap_file_content(&self, map_id: u32, content: &[u8]) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO osu_map_file_content (map_id, content) 
VALUES 
  ($1, $2) ON CONFLICT (map_id) DO 
UPDATE 
SET 
  content = $2"#,
            map_id as i32,
            content
        );

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }

    pub(super) async fn delete_beatmaps_of_beatmapset(
        tx: &mut Transaction<'_, Postgres>,
        mapset_id: u32,
    ) -> Result<HashMap<i32, Box<str>>> {
        let query = sqlx::query!(
            r#"
DELETE FROM
  osu_maps
WHERE
  mapset_id = $1
RETURNING
  map_id, checksum"#,
            mapset_id as i32
        );

        let mut rows = query.fetch(&mut **tx);
        let mut checksums = HashMap::new();

        while let Some(row) = rows.next().await {
            let row = row.wrap_err("Failed to fetch next")?;
            checksums.insert(row.map_id, row.checksum.into_boxed_str());
        }

        debug!("Deleted {} maps of mapset {mapset_id}", checksums.len());

        Ok(checksums)
    }

    pub(super) async fn upsert_beatmap(
        tx: &mut Transaction<'_, Postgres>,
        map: &BeatmapExtended,
        old_checksum: Option<&str>,
    ) -> Result<()> {
        let Some(ref checksum) = map.checksum else {
            warn!("Beatmap must contain checksum to be inserted into DB");

            return Ok(());
        };

        // `upsert_beatmap` is only called after `delete_beatmaps_of_beatmapset`
        // so we never need to update on conflict
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
  ) ON CONFLICT (map_id) DO NOTHING"#,
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

        query
            .execute(&mut **tx)
            .await
            .wrap_err("Failed to execute query")?;

        if old_checksum.is_some_and(|old| old != checksum) {
            let query = sqlx::query!(
                r#"
DELETE FROM 
  osu_map_file_content 
WHERE 
  map_id = $1"#,
                map.map_id as i32,
            );

            query
                .execute(&mut **tx)
                .await
                .wrap_err("Failed to delete from osu_map_file_content")?;
        }

        Ok(())
    }
}
