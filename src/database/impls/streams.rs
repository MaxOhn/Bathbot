use crate::{BotResult, Database};

use dashmap::DashMap;
use sqlx::{Done, Row};

impl Database {
    pub async fn add_stream_track(&self, channel: u64, user: u64) -> BotResult<bool> {
        let query = format!(
            "INSERT INTO stream_tracks VALUES ({},{}) ON CONFLICT DO NOTHING",
            channel, user
        );
        let done = sqlx::query(&query).execute(&self.pool).await?;

        Ok(done.rows_affected() > 0)
    }

    pub async fn get_stream_tracks(&self) -> BotResult<DashMap<u64, Vec<u64>>> {
        let tracks = sqlx::query("SELECT * FROM stream_tracks")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| (row.get::<i64, _>(1) as u64, row.get::<i64, _>(0) as u64))
            .fold(
                DashMap::with_capacity(100),
                |all: DashMap<u64, Vec<u64>>, (user, channel)| {
                    all.entry(user).or_default().push(channel);
                    all
                },
            );

        Ok(tracks)
    }

    pub async fn remove_channel_tracks(&self, channel: u64) -> BotResult<()> {
        let query = format!("DELETE FROM stream_tracks WHERE channel_id={}", channel);
        sqlx::query(&query).execute(&self.pool).await?;

        Ok(())
    }

    pub async fn remove_stream_track(&self, channel: u64, user: u64) -> BotResult<bool> {
        let query = format!(
            "DELETE FROM stream_tracks WHERE channel_id={} AND user_id={}",
            channel, user
        );
        let done = sqlx::query(&query).execute(&self.pool).await?;

        Ok(done.rows_affected() > 0)
    }
}
