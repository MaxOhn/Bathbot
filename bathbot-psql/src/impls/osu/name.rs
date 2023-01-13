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
            .wrap_err("failed to execute query")?;

        Ok(())
    }
}

#[cfg(test)]
pub(in crate::impls) mod tests {
    use eyre::Result;
    use futures::Future;

    use crate::{
        tests::{database, discord_id, osu_user_id, osu_username},
        Database,
    };

    use super::super::super::tests::user_config_wrap_upsert_delete;

    pub async fn wrap_upsert_delete<F>(psql: &Database, fut: F) -> Result<()>
    where
        F: Future<Output = Result<()>>,
    {
        let user_id = osu_user_id();
        let username = osu_username();

        psql.upsert_osu_username(user_id, username).await?;
        fut.await?;
        Database::delete_osu_username(psql, user_id).await?;

        Ok(())
    }

    #[tokio::test]
    async fn upsert_delete() -> Result<()> {
        let psql = database()?;

        wrap_upsert_delete(&psql, async { Ok(()) }).await
    }

    #[tokio::test]
    async fn select_by_osu_id() -> Result<()> {
        let psql = database()?;

        let fut = async {
            let user_id = osu_user_id();
            let username = osu_username();

            let name = psql.select_osu_name_by_osu_id(user_id).await?.unwrap();
            assert_eq!(name, username);

            Ok(())
        };

        wrap_upsert_delete(&psql, fut).await
    }

    #[tokio::test]
    async fn select_by_discord_id() -> Result<()> {
        let psql = database()?;

        let fut = async {
            let user_id = discord_id();

            let name = psql.select_osu_name_by_discord_id(user_id).await?;
            assert!(name.is_none());

            let fut = async {
                let username = osu_username();

                let name = psql.select_osu_name_by_discord_id(user_id).await?.unwrap();
                assert_eq!(name, username);

                Ok(())
            };

            wrap_upsert_delete(&psql, fut).await
        };

        user_config_wrap_upsert_delete(&psql, fut).await
    }
}
