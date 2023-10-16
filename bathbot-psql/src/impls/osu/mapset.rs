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

    pub(super) async fn update_beatmapset_compact<'c, E>(
        executor: E,
        mapset: &Beatmapset,
    ) -> Result<()>
    where
        E: Executor<'c, Database = Postgres>,
    {
        let query = sqlx::query!(
            r#"
UPDATE 
  osu_mapsets 
SET 
  user_id = $1, 
  artist = $2, 
  title = $3, 
  creator = $4, 
  source = $5, 
  video = $6, 
  rank_status = $7, 
  thumbnail = $8, 
  cover = $9, 
  last_update = NOW() 
WHERE 
  mapset_id = $10"#,
            mapset.creator_id as i32,
            mapset.artist,
            mapset.title,
            mapset.creator_name.as_str(),
            mapset.source,
            mapset.video,
            mapset.status as i16,
            mapset.covers.list,
            mapset.covers.cover,
            mapset.mapset_id as i32,
        );

        query
            .execute(executor)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }
}
