use crate::{
    bg_game::MapsetTags,
    database::{util::CustomSQL, MapsetTagWrapper},
    BotResult, Database,
};

use rayon::prelude::*;
use rosu::models::GameMode;
use sqlx::Row;
use std::{collections::HashSet, fmt::Write};
use twilight::model::id::UserId;

impl Database {
    pub async fn increment_bggame_score(&self, user_id: u64) -> BotResult<usize> {
        let query = format!(
            "
INSERT INTO
    bggame_stats
VALUES
    ({},1)
ON CONFLICT (discord_id) DO
    UPDATE
        SET score=score+1
RETURNING score
",
            user_id
        );
        let row = sqlx::query(&query).fetch_one(&self.pool).await?;
        Ok(row.get::<i64, _>(0) as usize)
    }

    pub async fn all_bggame_scores(&self) -> BotResult<Vec<(u64, u32)>> {
        let scores = sqlx::query("SELECT * FROM bggame_stats")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| (row.get::<i64, _>(0) as u64, row.get(1)))
            .collect();
        Ok(scores)
    }

    pub async fn get_bg_verified(&self) -> BotResult<HashSet<UserId>> {
        let users = sqlx::query("SELECT user_id FROM bg_verified")
            .fetch_all(&self.pool)
            .await?
            .into_par_iter()
            .map(|row| UserId(row.get::<i64, _>(0) as u64))
            .collect();
        Ok(users)
    }

    pub async fn add_tag_mapset(
        &self,
        mapset_id: u32,
        filetype: &str,
        mode: GameMode,
    ) -> BotResult<()> {
        let query = format!(
            "INSERT INTO map_tags VALUES ({},{},$1)",
            mapset_id, filetype
        );
        sqlx::query(&query)
            .bind(mode as i8)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_tags_mapset(
        &self,
        mapset_id: u32,
        tags: MapsetTags,
        value: bool,
    ) -> BotResult<()> {
        let mut query = String::from("UPDATE map_tags SET").set_tags(",", tags, value)?;
        write!(query, " WHERE beatmapset_id={}", mapset_id)?;
        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_tags_mapset(&self, mapset_id: u32) -> BotResult<MapsetTagWrapper> {
        let query = format!("SELECT * FROM map_tags WHERE beatmapset_id={}", mapset_id);
        let tags = sqlx::query_as(&query).fetch_one(&self.pool).await?;
        Ok(tags)
    }

    pub async fn get_all_tags_mapset(&self, mode: GameMode) -> BotResult<Vec<MapsetTagWrapper>> {
        let query = format!("SELECT * FROM map_tags WHERE mode={}", mode as u8);
        let tags = sqlx::query_as(&query).fetch_all(&self.pool).await?;
        Ok(tags)
    }

    pub async fn get_random_tags_mapset(&self, mode: GameMode) -> BotResult<MapsetTagWrapper> {
        let query = "
SELECT
    *
FROM
    map_tags AS mt
    JOIN (
        SELECT
            beatmapset_id
        FROM
            map_tags
        WHERE
            mode=$1
        ORDER BY
            RAND()
        LIMIT
            1
    ) as rndm ON mt.beatmapset_id = rndm.beatmapset_id
";
        Ok(sqlx::query_as(query)
            .bind(mode as i8)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn get_specific_tags_mapset(
        &self,
        mode: GameMode,
        included: MapsetTags,
        excluded: MapsetTags,
    ) -> BotResult<Vec<MapsetTagWrapper>> {
        if included.is_empty() && excluded.is_empty() {
            return self.get_all_tags_mapset(mode).await;
        }
        let mut query = String::from("SELECT * FROM map_tags WHERE mode=$1");
        query.push_str(" AND");
        if !included.is_empty() {
            query = query.set_tags(" AND ", included, true)?;
            if !excluded.is_empty() {
                query.push_str(" AND");
            }
        }
        if !excluded.is_empty() {
            query = query.set_tags(" AND ", excluded, false)?;
        }
        Ok(sqlx::query_as(&query)
            .bind(mode as i8)
            .fetch_all(&self.pool)
            .await?)
    }
}
