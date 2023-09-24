use std::{hash::BuildHasher, num::NonZeroU64};

use eyre::{Result, WrapErr};
use futures::StreamExt;
use rkyv::ser::{
    serializers::{AlignedSerializer, AllocSerializer},
    Serializer,
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
        let channels =
            rkyv::to_bytes::<_, 256>(channels).wrap_err("failed to serialize channels")?;

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

        let mut ser = AllocSerializer::<52>::default();

        ser.serialize_value(&channels)
            .wrap_err("failed to serialize channels")?;

        let (ser, scratch, shared) = ser.into_components();
        let mut channels_bytes = ser.into_inner();

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

        let prev_channels = unsafe { rkyv::archived_root::<Channels<S>>(&row.channels) };

        if !prev_channels.contains_key(&channel_id) {
            channels.extend(prev_channels.iter());

            // re-use the previous buffer
            channels_bytes.clear();
            let aligned_ser = AlignedSerializer::new(channels_bytes);
            let mut ser = AllocSerializer::new(aligned_ser, scratch, shared);

            ser.serialize_value(&channels)
                .wrap_err("failed to serialize updated channels")?;

            let channels_bytes = ser.into_serializer().into_inner();

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
