use std::{collections::HashMap, hash::BuildHasher};

use eyre::{Result, WrapErr};
use futures::StreamExt;
use rosu_pp::{
    catch::CatchDifficultyAttributes, mania::ManiaDifficultyAttributes,
    osu::OsuDifficultyAttributes, taiko::TaikoDifficultyAttributes, DifficultyAttributes,
};
use rosu_v2::prelude::{Beatmap, GameMode};
use sqlx::{Postgres, Transaction};

use crate::{
    model::osu::{
        DbBeatmap, DbBeatmapset, DbCatchDifficultyAttributes, DbManiaDifficultyAttributes,
        DbMapFilename, DbOsuDifficultyAttributes, DbTaikoDifficultyAttributes, MapVersion,
    },
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

        let Some(row) = row_opt else {
            return Ok(None)
        };

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
                DbMapFilename::Present(path)
            }
            (Some(path), None) => DbMapFilename::Present(path),
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
                    DbMapFilename::Present(path)
                }
                (Some(path), None) => DbMapFilename::Present(path),
                (Some(_), Some(_)) => DbMapFilename::ChecksumMismatch,
                (None, _) => DbMapFilename::Missing,
            };

            maps.insert(map.map_id, (map, mapset, filepath));
        }

        Ok(maps)
    }

    pub async fn select_map_difficulty_attrs(
        &self,
        map_id: u32,
        mode: GameMode,
        mods: u32,
    ) -> Result<Option<DifficultyAttributes>> {
        let attrs = match mode {
            GameMode::Osu => sqlx::query_as!(
                DbOsuDifficultyAttributes,
                r#"
SELECT 
  aim, 
  speed, 
  flashlight, 
  slider_factor, 
  speed_note_count, 
  ar, 
  od, 
  hp, 
  n_circles, 
  n_sliders, 
  n_spinners, 
  stars, 
  max_combo 
FROM 
  osu_map_difficulty 
WHERE 
  map_id = $1 
  AND mods = $2"#,
                map_id as i32,
                mods as i32
            )
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional osu difficulty")?
            .map(OsuDifficultyAttributes::from)
            .map(DifficultyAttributes::Osu),
            GameMode::Taiko => sqlx::query_as!(
                DbTaikoDifficultyAttributes,
                r#"
SELECT 
  stamina, 
  rhythm, 
  colour, 
  peak, 
  hit_window, 
  stars, 
  max_combo 
FROM 
  osu_map_difficulty_taiko 
WHERE 
  map_id = $1 
  AND mods = $2"#,
                map_id as i32,
                mods as i32
            )
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional taiko difficulty")?
            .map(TaikoDifficultyAttributes::from)
            .map(DifficultyAttributes::Taiko),
            GameMode::Catch => sqlx::query_as!(
                DbCatchDifficultyAttributes,
                r#"
SELECT 
  stars, 
  ar, 
  n_fruits, 
  n_droplets, 
  n_tiny_droplets 
FROM 
  osu_map_difficulty_catch 
WHERE 
  map_id = $1 
  AND mods = $2"#,
                map_id as i32,
                mods as i32
            )
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional catch difficulty")?
            .map(CatchDifficultyAttributes::from)
            .map(DifficultyAttributes::Catch),
            GameMode::Mania => sqlx::query_as!(
                DbManiaDifficultyAttributes,
                r#"
SELECT 
  stars, 
  hit_window, 
  max_combo 
FROM 
  osu_map_difficulty_mania 
WHERE 
  map_id = $1 
  AND mods = $2"#,
                map_id as i32,
                mods as i32
            )
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional mania difficulty")?
            .map(ManiaDifficultyAttributes::from)
            .map(DifficultyAttributes::Mania),
        };

        Ok(attrs)
    }

    pub async fn select_beatmap_file(&self, map_id: u32) -> Result<Option<String>> {
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
            .map(|row_opt| row_opt.map(|row| row.map_filepath))
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
  map_id, 
  map_version AS version
FROM 
  osu_maps 
WHERE 
  mapset_id = $1"#,
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

    pub(super) async fn upsert_beatmap(
        tx: &mut Transaction<'_, Postgres>,
        map: &Beatmap,
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

        // None if both map_id and checksum were the same i.e. no change
        let row_opt = query
            .fetch_optional(&mut *tx)
            .await
            .wrap_err("failed to fetch optional")?;

        let updated = row_opt
            .and_then(|row| row.inserted)
            .map_or(false, |inserted| !inserted);

        // Either new entry or different checksum
        // => map has changed so we should delete its attributes
        //    and all maps of its mapset
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
                .execute(&mut *tx)
                .await
                .wrap_err("failed to delete from osu_map_files")?;

            let query = match map.mode {
                GameMode::Osu => sqlx::query!(
                    r#"
DELETE FROM 
  osu_map_difficulty 
WHERE 
  map_id = $1"#,
                    map.map_id as i32
                ),
                GameMode::Taiko => sqlx::query!(
                    r#"
DELETE FROM 
  osu_map_difficulty_taiko 
WHERE 
  map_id = $1"#,
                    map.map_id as i32
                ),
                GameMode::Catch => sqlx::query!(
                    r#"
DELETE FROM 
  osu_map_difficulty_catch 
WHERE 
  map_id = $1"#,
                    map.map_id as i32
                ),
                GameMode::Mania => sqlx::query!(
                    r#"
DELETE FROM 
  osu_map_difficulty_mania 
WHERE 
  map_id = $1"#,
                    map.map_id as i32
                ),
            };

            query
                .execute(&mut *tx)
                .await
                .wrap_err("failed to delete from map difficulty")?;
        }

        Ok(())
    }

    pub async fn upsert_map_difficulty(
        &self,
        map_id: u32,
        mods: u32,
        attrs: &DifficultyAttributes,
    ) -> Result<()> {
        let query = match attrs {
            DifficultyAttributes::Osu(OsuDifficultyAttributes {
                aim,
                speed,
                flashlight,
                slider_factor,
                speed_note_count,
                ar,
                od,
                hp,
                n_circles,
                n_sliders,
                n_spinners,
                stars,
                max_combo,
            }) => sqlx::query!(
                r#"
INSERT INTO osu_map_difficulty (
  map_id, mods, aim, speed, flashlight, 
  slider_factor, speed_note_count, 
  ar, od, hp, n_circles, n_sliders, n_spinners, 
  stars, max_combo
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11, $12, $13, $14, $15
  ) ON CONFLICT (map_id, mods) DO 
UPDATE 
SET 
  aim = $3, 
  speed = $4, 
  flashlight = $5, 
  slider_factor = $6, 
  speed_note_count = $7, 
  ar = $8, 
  od = $9, 
  hp = $10, 
  n_circles = $11, 
  n_sliders = $12, 
  n_spinners = $13, 
  stars = $14, 
  max_combo = $15"#,
                map_id as i32,
                mods as i32,
                aim,
                speed,
                flashlight,
                slider_factor,
                speed_note_count,
                ar,
                od,
                hp,
                *n_circles as i32,
                *n_sliders as i32,
                *n_spinners as i32,
                stars,
                *max_combo as i32
            ),
            DifficultyAttributes::Taiko(TaikoDifficultyAttributes {
                stamina,
                rhythm,
                colour,
                peak,
                hit_window,
                stars,
                max_combo,
            }) => sqlx::query!(
                r#"
INSERT INTO osu_map_difficulty_taiko (
  map_id, mods, stamina, rhythm, colour, 
  peak, hit_window, stars, max_combo
) 
VALUES 
  ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (map_id, mods) DO 
UPDATE 
SET 
  stamina = $3, 
  rhythm = $4, 
  colour = $5, 
  peak = $6, 
  hit_window = $7, 
  stars = $8, 
  max_combo = $9"#,
                map_id as i32,
                mods as i32,
                stamina,
                rhythm,
                colour,
                peak,
                hit_window,
                stars,
                *max_combo as i32
            ),
            DifficultyAttributes::Catch(CatchDifficultyAttributes {
                stars,
                ar,
                n_fruits,
                n_droplets,
                n_tiny_droplets,
            }) => sqlx::query!(
                r#"
INSERT INTO osu_map_difficulty_catch (
  map_id, mods, stars, ar, n_fruits, n_droplets, 
  n_tiny_droplets
) 
VALUES 
  ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (map_id, mods) DO 
UPDATE 
SET 
  stars = $3, 
  ar = $4, 
  n_fruits = $5, 
  n_droplets = $6, 
  n_tiny_droplets = $7"#,
                map_id as i32,
                mods as i32,
                stars,
                ar,
                *n_fruits as i32,
                *n_droplets as i32,
                *n_tiny_droplets as i32
            ),
            DifficultyAttributes::Mania(ManiaDifficultyAttributes {
                stars,
                hit_window,
                max_combo,
            }) => sqlx::query!(
                r#"
INSERT INTO osu_map_difficulty_mania (
  map_id, mods, stars, hit_window, max_combo
) 
VALUES 
  ($1, $2, $3, $4, $5) ON CONFLICT (map_id, mods) DO 
UPDATE 
SET 
  stars = $3, 
  hit_window = $4, 
  max_combo = $5"#,
                map_id as i32,
                mods as i32,
                stars,
                hit_window,
                *max_combo as i32,
            ),
        };

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }
}
