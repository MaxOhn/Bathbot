use eyre::{Result, WrapErr};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;

use crate::{
    model::osu::{DbTrackedOsuUser, DbTrackedOsuUserInChannel},
    Database,
};

impl Database {
    pub async fn select_tracked_osu_users(&self) -> Result<Vec<DbTrackedOsuUser>> {
        let query = sqlx::query_as!(
            DbTrackedOsuUser,
            r#"
WITH pps AS (
  SELECT
    user_id,
    gamemode,
    pp as last_pp,
    last_updated
  FROM
    osu_users_100th_pp
  AS
    pps
)
SELECT
  *
FROM
  tracked_osu_users
JOIN
  pps
USING (user_id, gamemode)"#
        );

        query.fetch_all(self).await.wrap_err("Failed to fetch all")
    }

    pub async fn select_tracked_osu_users_channel(
        &self,
        channel_id: u64,
    ) -> Result<Vec<DbTrackedOsuUserInChannel>> {
        let query = sqlx::query_as!(
            DbTrackedOsuUserInChannel,
            r#"
SELECT
  user_id,
  gamemode,
  min_index,
  max_index,
  min_pp,
  max_pp,
  min_combo_percent,
  max_combo_percent
FROM
  tracked_osu_users
WHERE
  channel_id = $1"#,
            channel_id as i64
        );

        query.fetch_all(self).await.wrap_err("Failed to fetch all")
    }

    pub async fn upsert_tracked_osu_user(
        &self,
        user: &DbTrackedOsuUserInChannel,
        channel_id: u64,
    ) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO tracked_osu_users (
  user_id, gamemode, channel_id, min_index, max_index,
  min_pp, max_pp, min_combo_percent, max_combo_percent
)
VALUES
  ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT
  (user_id, gamemode, channel_id)
DO
  UPDATE
SET
    min_index = $4,
    max_index = $5,
    min_pp = $6,
    max_pp = $7,
    min_combo_percent = $8,
    max_combo_percent = $9"#,
            user.user_id,
            user.gamemode,
            channel_id as i64,
            user.min_index,
            user.max_index,
            user.min_pp,
            user.max_pp,
            user.min_combo_percent,
            user.max_combo_percent,
        );

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }

    pub async fn upsert_tracked_last_pp(
        &self,
        user_id: u32,
        mode: GameMode,
        pp: f32,
        now: OffsetDateTime,
    ) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO
  osu_users_100th_pp(user_id, gamemode, pp, last_updated)
VALUES
  ($1, $2, $3, $4)
ON CONFLICT
  (user_id, gamemode)
DO
  UPDATE
SET
  pp = $3,
  last_updated = $4"#,
            user_id as i32,
            mode as i16,
            pp,
            now,
        );

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }

    pub async fn delete_tracked_osu_user(
        &self,
        user_id: u32,
        mode: Option<GameMode>,
        channel_id: u64,
    ) -> Result<()> {
        let query = sqlx::query!(
            r#"
DELETE FROM 
  tracked_osu_users
WHERE
  user_id = $1
  AND ($2::INT2 is NULL OR gamemode = $2)
  AND channel_id = $3"#,
            user_id as i32,
            mode.map(|mode| mode as i16),
            channel_id as i64
        );

        // Note: We're never deleting from `osu_users_100th_pp` out of
        //       lazyness but that should be fine.

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }

    pub async fn delete_tracked_osu_channel(
        &self,
        channel_id: u64,
        mode: Option<GameMode>,
    ) -> Result<()> {
        let query = sqlx::query!(
            r#"
DELETE FROM 
  tracked_osu_users
WHERE 
  channel_id = $1
  AND ($2::INT2 IS NULL OR gamemode = $2)"#,
            channel_id as i64,
            mode.map(|mode| mode as i16) as _,
        );

        // Note: We're never deleting from `osu_users_100th_pp` out of
        //       lazyness but that should be fine.

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        Ok(())
    }
}
