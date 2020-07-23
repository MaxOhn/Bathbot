use crate::{
    database::{util::CustomSQL, StreamTrack},
    BotResult, Database,
};

use dashmap::DashMap;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;

impl Database {
    pub async fn add_stream_track(&self, channel: u64, user: u64) -> BotResult<()> {
        let query = format!("INSERT INTO stream_tracks VALUES ({},{})", channel, user);
        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_stream_tracks(&self) -> BotResult<HashMap<u64, Vec<u64>>> {
        let users: Vec<_> = sqlx::query("SELECT * FROM stream_tracks")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| (row.get::<i64, _>(1) as u64, row.get::<i64, _>(0) as u64))
            .collect();

        let len = users.len();
        let tracks = users.into_iter().fold(
            HashMap::with_capacity(len * 2 / 3),
            |mut all: HashMap<u64, Vec<u64>>, (user, channel)| {
                all.entry(user).or_default().push(channel);
                all
            },
        );
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
