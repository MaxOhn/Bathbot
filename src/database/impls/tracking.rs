use futures::stream::StreamExt;
use hashbrown::HashMap;
use rosu_v2::model::GameMode;
use serde_json::Value;
use time::OffsetDateTime;
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{
    database::TrackingUser, tracking::TrackingEntry, util::hasher::SimpleBuildHasher, BotResult,
    Database,
};

impl Database {
    #[cold]
    pub async fn get_osu_trackings(&self) -> BotResult<Vec<(TrackingEntry, TrackingUser)>> {
        let mut stream = sqlx::query!("SELECT * FROM osu_trackings").fetch(&self.pool);
        let mut tracks = Vec::with_capacity(10_000);

        while let Some(entry) = stream.next().await.transpose()? {
            let user_id = entry.user_id as u32;
            let mode = GameMode::from(entry.mode as u8);
            let last_top_score = entry.last_top_score;
            let channels: Value = entry.channels;

            let user = TrackingUser {
                user_id,
                mode,
                last_top_score,
                channels: serde_json::from_value(channels)?,
            };

            tracks.push((TrackingEntry { user_id, mode }, user));
        }

        Ok(tracks)
    }

    pub async fn update_osu_tracking_date(
        &self,
        entry: &TrackingEntry,
        last_top_score: OffsetDateTime,
    ) -> BotResult<()> {
        sqlx::query!(
            "UPDATE osu_trackings SET last_top_score=$3 WHERE user_id=$1 AND mode=$2",
            entry.user_id as i32,
            entry.mode as i16,
            last_top_score,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_osu_tracking(
        &self,
        user_id: u32,
        mode: GameMode,
        last_top_score: OffsetDateTime,
        channels: &HashMap<Id<ChannelMarker>, usize, SimpleBuildHasher>,
    ) -> BotResult<()> {
        sqlx::query!(
            "UPDATE osu_trackings \
            SET last_top_score=$3,channels=$4 \
            WHERE user_id=$1 AND mode=$2",
            user_id as i32,
            mode as i16,
            last_top_score,
            serde_json::to_value(&channels)?
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_osu_tracking(&self, user_id: u32, mode: GameMode) -> BotResult<()> {
        sqlx::query!(
            "DELETE FROM osu_trackings WHERE user_id=$1 AND mode=$2",
            user_id as i32,
            mode as i16
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_osu_tracking(
        &self,
        user_id: u32,
        mode: GameMode,
        last_top_score: OffsetDateTime,
        channel: Id<ChannelMarker>,
        limit: usize,
    ) -> BotResult<()> {
        let mut set = HashMap::with_hasher(SimpleBuildHasher);
        set.insert(channel, limit);

        let row = sqlx::query!(
            "INSERT INTO osu_trackings \
            VALUES ($1,$2,$3,$4)\
            ON CONFLICT (user_id,mode) DO \
            UPDATE \
            SET last_top_score=$3 \
            RETURNING channels",
            user_id as i32,
            mode as i16,
            last_top_score,
            serde_json::to_value(&set)?,
        )
        .fetch_one(&self.pool)
        .await?;

        let mut channels: HashMap<Id<ChannelMarker>, usize, SimpleBuildHasher> =
            serde_json::from_value(row.channels)?;

        if channels.insert(channel, limit).is_none() {
            sqlx::query!(
                "UPDATE osu_trackings SET channels=$3 WHERE user_id=$1 AND mode=$2",
                user_id as i32,
                mode as i16,
                serde_json::to_value(&channels)?
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }
}
