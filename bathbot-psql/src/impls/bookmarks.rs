use eyre::{Result, WrapErr};
use futures::StreamExt;
use twilight_model::id::{Id, marker::UserMarker};

use crate::{
    Database,
    model::osu::MapBookmark,
    util::{parse_genre, parse_language, parse_mode, parse_status},
};

impl Database {
    pub async fn select_user_bookmarks(&self, user_id: Id<UserMarker>) -> Result<Vec<MapBookmark>> {
        let query = sqlx::query!(
            r#"
SELECT 
  bookmarks.insert_date, 
  maps.map_id, 
  maps.mapset_id, 
  maps.user_id AS mapper_id, 
  maps.map_version, 
  maps.seconds_drain, 
  maps.seconds_total, 
  maps.count_circles, 
  maps.count_sliders, 
  maps.count_spinners, 
  maps.hp, 
  maps.cs, 
  maps.od, 
  maps.ar, 
  maps.bpm, 
  maps.gamemode, 
  mapsets.artist, 
  mapsets.title, 
  mapsets.creator, 
  mapsets.user_id AS creator_id, 
  mapsets.rank_status, 
  mapsets.ranked_date, 
  mapsets.genre_id, 
  mapsets.language_id, 
  mapsets.cover 
FROM 
  (
    SELECT 
      map_id, 
      insert_date 
    FROM 
      user_map_bookmarks 
    WHERE 
      user_id = $1
  ) AS bookmarks 
  JOIN (
    SELECT 
      map_id, 
      mapset_id, 
      user_id, 
      map_version, 
      seconds_drain, 
      seconds_total, 
      count_circles, 
      count_sliders, 
      count_spinners, 
      hp, 
      cs, 
      od, 
      ar, 
      bpm, 
      gamemode 
    FROM 
      osu_maps
  ) AS maps ON bookmarks.map_id = maps.map_id 
  JOIN (
    SELECT 
      mapset_id, 
      artist, 
      title, 
      creator, 
      user_id, 
      rank_status, 
      ranked_date, 
      genre_id, 
      language_id, 
      cover 
    FROM 
      osu_mapsets
  ) AS mapsets ON maps.mapset_id = mapsets.mapset_id 
ORDER BY 
  bookmarks.insert_date DESC"#,
            user_id.get() as i64
        );

        let mut rows = query.fetch(self);
        let mut bookmarks = Vec::new();

        while let Some(row_res) = rows.next().await {
            let row = row_res.wrap_err("Failed to fetch next")?;

            let bookmark = MapBookmark {
                insert_date: row.insert_date,
                map_id: row.map_id as u32,
                mapset_id: row.mapset_id as u32,
                mapper_id: row.mapper_id as u32,
                creator_id: row.creator_id as u32,
                creator_name: row.creator.into_boxed_str(),
                artist: row.artist.into_boxed_str(),
                title: row.title.into_boxed_str(),
                version: row.map_version.into_boxed_str(),
                mode: parse_mode(row.gamemode),
                hp: row.hp,
                cs: row.cs,
                od: row.od,
                ar: row.ar,
                bpm: row.bpm,
                count_circles: row.count_circles as u32,
                count_sliders: row.count_sliders as u32,
                count_spinners: row.count_spinners as u32,
                seconds_drain: row.seconds_drain as u32,
                seconds_total: row.seconds_total as u32,
                status: parse_status(row.rank_status),
                ranked_date: row.ranked_date,
                genre: parse_genre(row.genre_id),
                language: parse_language(row.language_id),
                cover_url: row.cover.into_boxed_str(),
            };

            bookmarks.push(bookmark);
        }

        Ok(bookmarks)
    }

    pub async fn insert_user_bookmark(&self, user_id: Id<UserMarker>, map_id: u32) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO user_map_bookmarks (user_id, map_id) 
VALUES 
  ($1, $2) ON CONFLICT (user_id, map_id) DO NOTHING"#,
            user_id.get() as i64,
            map_id as i32
        );

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }

    pub async fn delete_user_bookmark(&self, user_id: Id<UserMarker>, map_id: u32) -> Result<()> {
        let query = sqlx::query!(
            r#"
DELETE FROM 
  user_map_bookmarks 
WHERE 
  user_id = $1 
  AND map_id = $2"#,
            user_id.get() as i64,
            map_id as i32
        );

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }
}
