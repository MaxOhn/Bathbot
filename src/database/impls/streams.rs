use crate::{BotResult, Database};

use dashmap::DashMap;
use futures::stream::StreamExt;

impl Database {
    pub async fn add_stream_track(&self, channel: u64, user: u64) -> BotResult<bool> {
        let done = sqlx::query!(
            "INSERT INTO stream_tracks VALUES ($1,$2) ON CONFLICT DO NOTHING",
            channel as i64,
            user as i64
        )
        .execute(&self.pool)
        .await?;

        Ok(done.rows_affected() > 0)
    }

    #[cold]
    pub async fn get_stream_tracks(&self) -> BotResult<DashMap<u64, Vec<u64>>> {
        let mut stream = sqlx::query!("SELECT * FROM stream_tracks").fetch(&self.pool);
        let tracks: DashMap<_, Vec<_>> = DashMap::with_capacity(1000);

        while let Some(entry) = stream.next().await.transpose()? {
            let channel_id: i64 = entry.channel_id;
            let user_id: i64 = entry.user_id;

            tracks
                .entry(user_id as u64)
                .or_default()
                .push(channel_id as u64);
        }

        Ok(tracks)
    }

    pub async fn remove_channel_tracks(&self, channel: u64) -> BotResult<()> {
        sqlx::query!(
            "DELETE FROM stream_tracks WHERE channel_id=$1",
            channel as i64,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_stream_track(&self, channel: u64, user: u64) -> BotResult<bool> {
        let done = sqlx::query!(
            "DELETE FROM stream_tracks WHERE channel_id=$1 AND user_id=$2",
            channel as i64,
            user as i64,
        )
        .execute(&self.pool)
        .await?;

        Ok(done.rows_affected() > 0)
    }
}
