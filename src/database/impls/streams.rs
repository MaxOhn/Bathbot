use std::iter;

use eyre::Result;
use flurry::HashMap as FlurryMap;
use futures::stream::StreamExt;

use crate::{util::hasher::SimpleBuildHasher, Database};

type TrackedStreams = FlurryMap<u64, Vec<u64>, SimpleBuildHasher>;

impl Database {
    pub async fn add_stream_track(&self, channel: u64, user: u64) -> Result<bool> {
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
    pub async fn get_stream_tracks(&self) -> Result<TrackedStreams> {
        let mut stream = sqlx::query!("SELECT * FROM stream_tracks").fetch(&self.pool);
        let tracks = TrackedStreams::with_capacity_and_hasher(1000, SimpleBuildHasher);

        {
            let guard = tracks.guard();

            while let Some(entry) = stream.next().await.transpose()? {
                let channel_id = entry.channel_id as u64;
                let user_id = entry.user_id as u64;

                let missing = tracks
                    .compute_if_present(
                        &user_id,
                        |_, channels| {
                            let channels = channels.iter().copied().chain(iter::once(channel_id));

                            Some(channels.collect())
                        },
                        &guard,
                    )
                    .is_none();

                if missing {
                    tracks.insert(user_id, vec![channel_id], &guard);
                }
            }
        }

        Ok(tracks)
    }

    pub async fn remove_channel_tracks(&self, channel: u64) -> Result<()> {
        sqlx::query!(
            "DELETE FROM stream_tracks WHERE channel_id=$1",
            channel as i64,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_stream_track(&self, channel: u64, user: u64) -> Result<bool> {
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
