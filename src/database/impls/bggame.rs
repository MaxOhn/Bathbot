use crate::{
    database::{mode_enum, MapsetTagWrapper},
    util::bg_game::MapsetTags,
    BotResult, Database,
};

use postgres_types::Type;
use rosu::models::GameMode;
use std::collections::HashSet;
use twilight::model::id::UserId;

impl Database {
    pub async fn increment_bggame_score(&self, user_id: u64) -> BotResult<()> {
        let query = "
INSERT INTO
    bggame_stats
VALUES
    ($1,1)
ON CONFLICT DO
    UPDATE
        SET score=score+1
";
        let client = self.pool.get().await?;
        let statement = client.prepare_typed(query, &[Type::INT8]).await?;
        client.execute(&statement, &[&(user_id as i64)]).await?;
        Ok(())
    }

    pub async fn get_bggame_score(&self, user_id: u64) -> BotResult<u32> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "SELECT score FROM bggame_stats WHERE discord_id=$1",
                &[Type::INT8],
            )
            .await?;
        let score = client
            .query_one(&statement, &[&(user_id as i64)])
            .await?
            .map_or(0, |row| row.get(0));
        Ok(score)
    }

    pub async fn all_bggame_scores(&self) -> BotResult<Vec<(u64, u32)>> {
        let client = self.pool.get().await?;
        let statement = client.prepare("SELECT * FROM bggame_stats").await?;
        let scores = client
            .query(&statement)
            .await?
            .into_iter()
            .map(|row| (row.get(0), row.get(1)))
            .collect();
        Ok(scores)
    }

    pub async fn get_bg_verified(&self) -> BotResult<HashSet<UserId>> {
        let client = self.pool.get().await?;
        let statement = client.prepare("SELECT user_id FROM bg_verified").await?;
        let users = client
            .query(&statement)
            .await?
            .into_iter()
            .map(|row| UserId(row.get(0)))
            .collect();
        Ok(users)
    }

    pub async fn add_tag_mapset(
        &self,
        mapset_id: u32,
        filetype: &str,
        mode: GameMode,
    ) -> BotResult<()> {
        let query = "
INSERT
    INTO map_tags
VALUES
    ($1,$2,$3)
";
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(query, &[Type::INT4, Type::BYTEA, Type::INT2])
            .await?;
        client
            .execute(&statement, &[&(mapset_id as i32), filetype, mode as i8])
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
        let client = self.pool.get().await?;
        let statement = client.prepare(&query).await?;
        client.execute(&statement).await?;
        Ok(())
    }

    pub async fn get_tags_mapset(&self, mapset_id: u32) -> BotResult<MapsetTagWrapper> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "SELECT * FROM map_tags WHERE beatmapset_id=$1",
                &[Type::INT4],
            )
            .await?;
        let tags = client
            .query_one(statement, &[mapset_id as i64])
            .await?
            .into();
        Ok(tags)
    }

    pub async fn get_all_tags_mapset(&self, mode: GameMode) -> BotResult<Vec<MapsetTagWrapper>> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed("SELECT * FROM map_tags WHERE mode=$1", &[Type::BYTEA])
            .await?;
        let tags = client
            .query(statement, &[mode_enum(mode)])
            .await?
            .into_iter()
            .map(|row| row.into())
            .collect();
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
        let client = self.pool.get().await?;
        let statement = client.prepare_typed(query, &[Type::BYTEA]).await?;
        let tags = client
            .query_one(statement, &[mode_enum(mode)])
            .await?
            .into();
        Ok(tags)
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
        let mut query = format!("SELECT * FROM map_tags WHERE mode={}", mode_enum(mode));
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
        let client = self.pool.get().await?;
        let statement = client.prepare(&query).await?;
        let mapsets = client
            .query(statement)
            .await?
            .into_iter()
            .map(|row| row.into())
            .collect();
        Ok(mapsets)
    }
}
