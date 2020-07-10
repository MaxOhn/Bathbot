use crate::{
    database::{util::CustomSQL, StreamTrack},
    BotResult, Database,
};

use sqlx::Row;
use std::collections::{HashMap, HashSet};

impl Database {
    pub async fn add_twitch_user(&self, user_id: u64, name: &str) -> BotResult<()> {
        sqlx::query("INSERT INTO twitch_users VALUES (&1,$2)")
            .bind(user_id as i64)
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn add_stream_track(&self, channel: u64, user: u64) -> BotResult<()> {
        let query = format!("INSERT INTO stream_tracks VALUES ({},{})", channel, user);
        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_twitch_users(&self, user_ids: &[u64]) -> BotResult<HashMap<String, u64>> {
        // TODO: Check how long twitch ids are
        let query = String::from("SELECT * FROM twitch_users WHERE user_id IN").in_clause(user_ids);
        let users = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| (row.get(1), row.get::<i64, _>(0) as u64))
            .collect();
        Ok(users)
    }

    pub async fn get_stream_tracks(&self) -> BotResult<HashSet<StreamTrack>> {
        let tracks = sqlx::query_as::<_, StreamTrack>("SELECT * FROM stream_tracks")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .collect();
        Ok(tracks)
    }

    pub async fn remove_stream_track(&self, channel: u64, user: u64) -> BotResult<()> {
        let query = format!(
            "
DELETE FROM
    stream_tracks
WHERE
    channel_id={}
    AND user_id={}
",
            channel, user
        );
        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }
}
