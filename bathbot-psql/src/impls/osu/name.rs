use eyre::{Result, WrapErr};
use rosu_v2::prelude::Username;
use sqlx::{Executor, Postgres};
use twilight_model::id::{marker::UserMarker, Id};

use crate::database::Database;

impl Database {
    pub async fn select_osu_name_by_osu_id(&self, user_id: u32) -> Result<Option<Username>> {
        let query = sqlx::query!(
            r#"
SELECT 
  username 
FROM 
  osu_user_names 
WHERE 
  user_id = $1"#,
            user_id as i32
        );

        let row_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?;

        Ok(row_opt.map(|row| row.username.into()))
    }

    /// Be sure wildcards (_, %) are escaped as required!
    pub async fn select_osu_id_by_osu_name(
        &self,
        username: &str,
        alt_username: Option<&str>,
    ) -> Result<Option<u32>> {
        let query = sqlx::query!(
            r#"
SELECT 
  user_id 
FROM 
  osu_user_names 
WHERE 
  username ILIKE $1 OR username ILIKE $2"#,
            username,
            alt_username,
        );

        let row_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?;

        Ok(row_opt.map(|row| row.user_id as u32))
    }

    pub async fn select_osu_name_by_discord_id(
        &self,
        user_id: Id<UserMarker>,
    ) -> Result<Option<Username>> {
        let query = sqlx::query!(
            r#"
SELECT 
  username 
FROM 
  osu_user_names 
WHERE 
  user_id = (
    SELECT 
      osu_id 
    FROM 
      user_configs 
    WHERE 
      discord_id = $1
  )"#,
            user_id.get() as i64
        );

        let row_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?;

        Ok(row_opt.map(|row| row.username.into()))
    }

    pub async fn upsert_osu_username(&self, user_id: u32, username: &str) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO osu_user_names (user_id, username) 
VALUES 
  ($1, $2) ON CONFLICT (user_id) DO 
UPDATE 
SET 
  username = $2"#,
            user_id as i32,
            username,
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }

    pub async fn delete_osu_username<'c, E>(executor: E, user_id: u32) -> Result<()>
    where
        E: Executor<'c, Database = Postgres>,
    {
        let query = sqlx::query!(
            r#"
DELETE FROM 
  osu_user_names 
WHERE 
  user_id = $1"#,
            user_id as i32
        );

        query
            .execute(executor)
            .await
            .wrap_err("Failed to execute names query")?;

        Ok(())
    }
}
