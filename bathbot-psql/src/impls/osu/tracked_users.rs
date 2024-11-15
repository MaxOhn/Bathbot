use std::{hash::BuildHasher, num::NonZeroU64};

use eyre::{Result, WrapErr};
use futures::StreamExt;
use rkyv::{
    rancor::BoxedError,
    ser::Serializer,
    with::{ArchiveWith, AsVec, With},
};
use rosu_v2::prelude::GameMode;

use crate::{
    model::osu::{Channels, DbTrackedOsuUser, TrackedOsuUserKey, TrackedOsuUserValue},
    Database,
};

impl Database {
    pub async fn select_tracked_osu_users<S>(
        &self,
    ) -> Result<Vec<(TrackedOsuUserKey, TrackedOsuUserValue<S>)>>
    where
        S: Default + BuildHasher,
    {
        let query = sqlx::query_as!(
            DbTrackedOsuUser,
            r#"
SELECT 
  user_id, 
  gamemode, 
  channels, 
  last_update 
FROM 
  tracked_osu_users"#
        );

        let mut rows = query.fetch(self);
        let mut tracks = Vec::with_capacity(16_000);

        while let Some(row_res) = rows.next().await {
            let row = row_res.wrap_err("failed to fetch next")?;
            let (key, value) = row.into();
            tracks.push((key, value));
        }

        Ok(tracks)
    }

    pub async fn update_tracked_osu_user_date(&self, user_id: u32, mode: GameMode) -> Result<()> {
        let query = sqlx::query!(
            r#"
UPDATE
  tracked_osu_users
SET
  last_update = NOW()
WHERE
  user_id = $1
  AND gamemode = $2"#,
            user_id as i32,
            mode as i16,
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }

    pub async fn update_tracked_osu_user_channels<S>(
        &self,
        user_id: u32,
        mode: GameMode,
        channels: &Channels<S>,
    ) -> Result<()> {
        let channels = rkyv::util::with_arena(|arena| {
            let mut writer = Vec::new();
            let mut serializer = Serializer::new(&mut writer, arena.acquire(), ());
            rkyv::api::serialize_using(With::<_, AsVec>::cast(channels), &mut serializer)?;

            Ok::<_, BoxedError>(writer)
        })
        .wrap_err("Failed to serialize channels")?;

        let query = sqlx::query!(
            r#"
UPDATE 
  tracked_osu_users 
SET 
  channels = $3 
WHERE 
  user_id = $1 
  AND gamemode = $2"#,
            user_id as i32,
            mode as i16,
            &channels as &[u8],
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }

    pub async fn delete_tracked_osu_user_by_mode(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Result<()> {
        let query = sqlx::query!(
            r#"
DELETE FROM 
  tracked_osu_users 
WHERE 
  user_id = $1 
  AND gamemode = $2"#,
            user_id as i32,
            mode as i16,
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }

    pub async fn insert_osu_tracking<S>(
        &self,
        user_id: u32,
        mode: GameMode,
        channel_id: NonZeroU64,
        limit: u8,
    ) -> Result<()>
    where
        S: Default + BuildHasher,
    {
        let mut tx = self.begin().await.wrap_err("failed to begin transaction")?;

        let mut channels = Channels::with_capacity_and_hasher(1, S::default());
        channels.insert(channel_id, limit);

        let mut channels_bytes = rkyv::util::with_arena(|arena| {
            let mut writer = Vec::new();
            let mut serializer = Serializer::new(&mut writer, arena.acquire(), ());
            rkyv::api::serialize_using(With::<_, AsVec>::cast(&channels), &mut serializer)?;

            Ok::<_, BoxedError>(writer)
        })
        .wrap_err("Failed to serialize channels")?;

        let query = sqlx::query!(
            r#"
INSERT INTO tracked_osu_users (user_id, gamemode, channels) 
VALUES 
  ($1, $2, $3) ON CONFLICT (user_id, gamemode) DO 
UPDATE 
SET 
  last_update = NOW() RETURNING channels"#,
            user_id as i32,
            mode as i16,
            &channels_bytes as &[u8],
        );

        let row = query
            .fetch_one(&mut *tx)
            .await
            .wrap_err("failed to fetch one")?;

        let prev_channels =
            rkyv::access::<<AsVec as ArchiveWith<Channels<S>>>::Archived, BoxedError>(
                &row.channels,
            )
            .wrap_err("Failed to validate channels")?;

        if !prev_channels.iter().any(|entry| entry.key == channel_id) {
            channels.extend(
                prev_channels
                    .iter()
                    .map(|entry| (entry.key.to_native(), entry.value)),
            );

            // re-use the previous buffer
            rkyv::util::with_arena(|arena| {
                channels_bytes.clear();
                let mut serializer = Serializer::new(&mut channels_bytes, arena.acquire(), ());
                rkyv::api::serialize_using(With::<_, AsVec>::cast(&channels), &mut serializer)?;

                Ok::<_, BoxedError>(())
            })
            .wrap_err("Failed to serialize updated channels")?;

            let query = sqlx::query!(
                r#"
UPDATE 
  tracked_osu_users 
SET 
  channels = $3 
WHERE 
  user_id = $1 
  AND gamemode = $2"#,
                user_id as i32,
                mode as i16,
                &channels_bytes as &[u8],
            );

            query
                .execute(&mut *tx)
                .await
                .wrap_err("failed to execute query")?;
        }

        tx.commit().await.wrap_err("failed to commit transaction")?;

        Ok(())
    }
}
