use std::{
    collections::{hash_map::Entry, HashMap},
    hash::BuildHasher,
};

use eyre::{Result, WrapErr};
use futures::StreamExt;
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::database::Database;

impl Database {
    pub async fn select_tracked_twitch_streams<S>(
        &self,
    ) -> Result<HashMap<u64, Vec<Id<ChannelMarker>>, S>>
    where
        S: Default + BuildHasher,
    {
        let query = sqlx::query!(
            r#"
SELECT 
  channel_id, 
  user_id 
FROM 
  tracked_twitch_streams"#
        );

        let mut rows = query.fetch(self);
        let mut tracks = HashMap::with_capacity_and_hasher(1000, S::default());

        while let Some(row_res) = rows.next().await {
            let row = row_res.wrap_err("failed to fetch next")?;
            let channel_id = Id::new(row.channel_id as u64);
            let user_id = row.user_id as u64;

            // match instead of `.or_insert_with(...).push(...)` to avoid bounds check
            match tracks.entry(user_id) {
                Entry::Vacant(e) => {
                    e.insert(vec![channel_id]);
                }
                Entry::Occupied(mut e) => e.get_mut().push(channel_id),
            }
        }

        Ok(tracks)
    }

    /// Returns whether a new entry was inserted
    pub async fn insert_tracked_twitch_stream(
        &self,
        channel: Id<ChannelMarker>,
        user: u64,
    ) -> Result<bool> {
        let query = sqlx::query!(
            r#"
INSERT INTO tracked_twitch_streams (channel_id, user_id) 
VALUES 
  ($1, $2) ON CONFLICT (channel_id, user_id) DO NOTHING"#,
            channel.get() as i64,
            user as i64,
        );

        let res = query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(res.rows_affected() > 0)
    }

    pub async fn delete_tracked_twitch_streams(&self, channel: Id<ChannelMarker>) -> Result<()> {
        let query = sqlx::query!(
            r#"
DELETE FROM 
  tracked_twitch_streams 
WHERE 
  channel_id = $1"#,
            channel.get() as i64,
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }

    /// Returns whether an entry was deleted
    pub async fn delete_tracked_twitch_stream(
        &self,
        channel: Id<ChannelMarker>,
        user: u64,
    ) -> Result<bool> {
        let query = sqlx::query!(
            r#"
DELETE FROM 
  tracked_twitch_streams 
WHERE 
  channel_id = $1 
  AND user_id = $2"#,
            channel.get() as i64,
            user as i64,
        );

        let res = query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(res.rows_affected() > 0)
    }
}
