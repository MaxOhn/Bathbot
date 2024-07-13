use eyre::{Result, WrapErr};
use rosu_v2::prelude::{Beatmapset, BeatmapsetExtended};
use sqlx::{Executor, Postgres};

use crate::{
    model::osu::{ArtistTitle, DbBeatmapset},
    Database,
};

impl Database {
    pub async fn select_mapset(&self, mapset_id: u32) -> Result<Option<DbBeatmapset>> {
        let query = sqlx::query_as!(
            DbBeatmapset,
            r#"
SELECT 
  mapset_id, 
  user_id, 
  artist, 
  title, 
  creator, 
  rank_status, 
  ranked_date, 
  thumbnail, 
  cover 
FROM 
  osu_mapsets 
WHERE 
  mapset_id = $1"#,
            mapset_id as i32
        );

        query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")
    }

    pub async fn select_mapset_artist_title(&self, mapset_id: u32) -> Result<Option<ArtistTitle>> {
        let query = sqlx::query_as!(
            ArtistTitle,
            r#"
SELECT 
  artist, 
  title 
FROM 
  osu_mapsets 
WHERE 
  mapset_id = $1"#,
            mapset_id as i32
        );

        query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")
    }

    pub async fn upsert_beatmapset(&self, mapset: &BeatmapsetExtended) -> Result<()> {
        let mut tx = self.begin().await.wrap_err("failed to begin transaction")?;

        let query = sqlx::query!(
            r#"
INSERT INTO osu_mapsets (
  mapset_id, user_id, artist, title, 
  creator, source, tags, video, storyboard, 
  bpm, rank_status, ranked_date, genre_id, 
  language_id, thumbnail, cover
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11, $12, $13, $14, $15, $16
  ) ON CONFLICT (mapset_id) DO 
UPDATE 
SET 
  user_id = $2, 
  artist = $3, 
  title = $4, 
  creator = $5, 
  source = $6, 
  tags = $7, 
  video = $8, 
  storyboard = $9, 
  bpm = $10, 
  rank_status = $11, 
  ranked_date = $12, 
  genre_id = $13, 
  language_id = $14, 
  thumbnail = $15, 
  cover = $16, 
  last_update = NOW()"#,
            mapset.mapset_id as i32,
            mapset.creator_id as i32,
            mapset.artist,
            mapset.title,
            mapset.creator.as_ref().map(|user| user.username.as_str()),
            mapset.source,
            mapset.tags,
            mapset.video,
            mapset.storyboard,
            mapset.bpm,
            mapset.status as i16,
            mapset.ranked_date,
            mapset.genre.map(|genre| genre as i16),
            mapset.language.map(|language| language as i16),
            mapset.covers.list,
            mapset.covers.cover,
        );

        query
            .execute(&mut *tx)
            .await
            .wrap_err("failed to execute query")?;

        if let Some(ref maps) = mapset.maps {
            for map in maps {
                Self::upsert_beatmap(&mut tx, map)
                    .await
                    .wrap_err("failed to insert map")?;
            }
        }

        tx.commit().await.wrap_err("failed to commit transaction")?;

        Ok(())
    }

    pub(super) async fn update_beatmapsets<'c, E>(
        executor: E,
        mapsets: impl Iterator<Item = &Beatmapset>,
        len: usize,
    ) -> Result<()>
    where
        E: Executor<'c, Database = Postgres>,
    {
        let mut vec_creator_id = Vec::with_capacity(len);
        let mut vec_artist = Vec::with_capacity(len);
        let mut vec_title = Vec::with_capacity(len);
        let mut vec_creator_name = Vec::with_capacity(len);
        let mut vec_source = Vec::with_capacity(len);
        let mut vec_video = Vec::with_capacity(len);
        let mut vec_status = Vec::with_capacity(len);
        let mut vec_thumbnail = Vec::with_capacity(len);
        let mut vec_cover = Vec::with_capacity(len);
        let mut vec_mapset_id = Vec::with_capacity(len);

        for mapset in mapsets {
            vec_creator_id.push(mapset.creator_id as i32);
            vec_artist.push(mapset.artist.as_str());
            vec_title.push(mapset.title.as_str());
            vec_creator_name.push(mapset.creator_name.as_str());
            vec_source.push(mapset.source.as_str());
            vec_video.push(mapset.video);
            vec_status.push(mapset.status as i16);
            vec_thumbnail.push(mapset.covers.list.as_str());
            vec_cover.push(mapset.covers.cover.as_str());
            vec_mapset_id.push(mapset.mapset_id as i32);
        }

        let query = sqlx::query!(
            r#"
UPDATE 
  osu_mapsets 
SET 
  user_id = bulk.user_id, 
  artist = bulk.artist, 
  title = bulk.title, 
  creator = bulk.creator, 
  source = bulk.source, 
  video = bulk.video, 
  rank_status = bulk.rank_status, 
  thumbnail = bulk.thumbnail, 
  cover = bulk.cover, 
  last_update = NOW() 
FROM 
  (
    SELECT
      *
    FROM
      UNNEST(
        $1::INT4[], $2::VARCHAR[], $3::VARCHAR[], $4::VARCHAR[], 
        $5::VARCHAR[], $6::BOOL[], $7::INT2[], $8::VARCHAR[], 
        $9::VARCHAR[], $10::INT4[]
      ) AS t(
        user_id, artist, title, creator, source, video, 
        rank_status, thumbnail, cover, mapset_id
      )
  ) AS bulk
WHERE 
  osu_mapsets.mapset_id = bulk.mapset_id"#,
            &vec_creator_id,
            &vec_artist as _,
            &vec_title as _,
            &vec_creator_name as _,
            &vec_source as _,
            &vec_video,
            &vec_status,
            &vec_thumbnail as _,
            &vec_cover as _,
            &vec_mapset_id,
        );

        query
            .execute(executor)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }
}
