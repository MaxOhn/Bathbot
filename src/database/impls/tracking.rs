use crate::{database::TrackingUser, BotResult, Database};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use rosu::models::GameMode;
use sqlx::{types::Json, Row};
use std::collections::HashSet;
use twilight::model::id::ChannelId;

impl Database {
    pub async fn get_osu_trackings(&self) -> BotResult<DashMap<(u32, GameMode), TrackingUser>> {
        let tracks = sqlx::query_as("SELECT * FROM osu_tracking")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|user: TrackingUser| ((user.user_id, user.mode), user))
            .collect();
        Ok(tracks)
    }

    pub async fn update_osu_tracking(
        &self,
        user_id: u32,
        mode: GameMode,
        last_top_score: DateTime<Utc>,
        channels: &HashSet<ChannelId>,
    ) -> BotResult<()> {
        let query = "
UPDATE
    osu_tracking
SET
    last_top_score=$3, channels=$4
WHERE
    user_id=$1 AND mode=$2";
        sqlx::query(query)
            .bind(user_id)
            .bind(mode as i8)
            .bind(last_top_score)
            .bind(Json(channels))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_osu_tracking(&self, user_id: u32, mode: GameMode) -> BotResult<()> {
        let query = "DELETE FROM osu_tracking WHERE user_id=$1 AND mode=$2";
        sqlx::query(query)
            .bind(user_id)
            .bind(mode as i8)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_osu_tracking(
        &self,
        user_id: u32,
        mode: GameMode,
        last_top_score: DateTime<Utc>,
        channel: u64,
    ) -> BotResult<()> {
        let query = "
INSERT INTO
    osu_tracking
VALUES
    ($1,$2,$3,$4)
ON CONFLICT
    (user_id, mode)
UPDATE SET
    last_top_score=$3
RETURNING channels";
        let mut set = HashSet::with_capacity(1);
        set.insert(channel);
        let mut channels: Json<HashSet<i64>> = sqlx::query(query)
            .bind(user_id)
            .bind(mode as i8)
            .bind(last_top_score)
            .bind(Json(set))
            .fetch_one(&self.pool)
            .await?
            .get(0);
        if channels.insert(channel as i64) {
            let query = "UPDATE osu_tracking SET channels=$3 WHERE user_id=$1 AND mode=$2";
            sqlx::query(query)
                .bind(user_id)
                .bind(mode as i8)
                .bind(Json(channels))
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}
