use eyre::{Result, WrapErr};
use rosu_v2::prelude::{GameMode, Username};
use time::UtcOffset;
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    model::configs::{
        DbUserConfig, ListSize, MinimizedPp, OsuUserId, OsuUsername, ScoreSize, UserConfig,
    },
    Database,
};

impl Database {
    pub async fn select_user_config_with_osu_id_by_discord_id(
        &self,
        user_id: Id<UserMarker>,
    ) -> Result<Option<UserConfig<OsuUserId>>> {
        let query = sqlx::query_as!(
            DbUserConfig,
            r#"
SELECT 
  score_size, 
  list_size, 
  minimized_pp, 
  gamemode, 
  osu_id, 
  show_retries, 
  twitch_id, 
  timezone_seconds 
FROM 
  user_configs 
WHERE 
  discord_id = $1"#,
            user_id.get() as i64,
        );

        let config_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?;

        Ok(config_opt.map(UserConfig::from))
    }

    pub async fn select_user_config_with_osu_name_by_discord_id(
        &self,
        user_id: Id<UserMarker>,
    ) -> Result<Option<UserConfig<OsuUsername>>> {
        let query = sqlx::query!(
            r#"
SELECT 
  score_size, 
  list_size, 
  minimized_pp, 
  gamemode, 
  (
    SELECT 
      username 
    FROM 
      osu_user_names 
    WHERE 
      user_id = osu_id
  ), 
  show_retries, 
  twitch_id, 
  timezone_seconds 
FROM 
  user_configs 
WHERE 
  discord_id = $1"#,
            user_id.get() as i64,
        );

        let config_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?
            .map(|row| UserConfig {
                score_size: row.score_size.map(ScoreSize::try_from).and_then(Result::ok),
                list_size: row.list_size.map(ListSize::try_from).and_then(Result::ok),
                minimized_pp: row
                    .minimized_pp
                    .map(MinimizedPp::try_from)
                    .and_then(Result::ok),
                mode: row.gamemode.map(|mode| GameMode::from(mode as u8)),
                osu: row.username.map(Username::from),
                show_retries: row.show_retries,
                twitch_id: row.twitch_id.map(|id| id as u64),
                timezone: row
                    .timezone_seconds
                    .map(UtcOffset::from_whole_seconds)
                    .map(Result::unwrap),
            });

        Ok(config_opt)
    }

    pub async fn select_osu_id_by_discord_id(
        &self,
        user_id: Id<UserMarker>,
    ) -> Result<Option<u32>> {
        let query = sqlx::query!(
            r#"
SELECT 
  osu_id 
FROM 
  user_configs 
WHERE 
  discord_id = $1"#,
            user_id.get() as i64
        );

        let osu_id = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?
            .and_then(|row| row.osu_id);

        Ok(osu_id.map(|id| id as u32))
    }

    pub async fn select_skin_url(&self, user_id: Id<UserMarker>) -> Result<Option<String>> {
        let query = sqlx::query!(
            r#"
SELECT 
  skin_url 
FROM 
  user_configs 
WHERE 
  discord_id = $1"#,
            user_id.get() as i64
        );

        query
            .fetch_optional(self)
            .await
            .map(|row| row.and_then(|row| row.skin_url))
            .wrap_err("failed to fetch optional")
    }

    pub async fn select_skin_url_by_osu_id(&self, user_id: u32) -> Result<Option<String>> {
        let query = sqlx::query!(
            r#"
SELECT 
  skin_url 
FROM 
  user_configs 
WHERE 
  osu_id = $1 
  AND skin_url IS NOT NULL"#,
            user_id as i32
        );

        query
            .fetch_optional(self)
            .await
            .map(|row| row.and_then(|row| row.skin_url))
            .wrap_err("failed to fetch optional")
    }

    /// Be sure wildcards (_, %) are escaped as required!
    pub async fn select_skin_url_by_osu_name(&self, username: &str) -> Result<Option<String>> {
        let query = sqlx::query!(
            r#"
SELECT 
  skin_url 
FROM 
  (
    SELECT 
      skin_url, 
      osu_id 
    FROM 
      user_configs 
    WHERE 
      skin_url IS NOT NULL 
      AND osu_id IS NOT NULL
  ) AS configs 
  JOIN (
    SELECT 
      user_id 
    FROM 
      osu_user_names 
    WHERE 
      username ILIKE $1
  ) AS names ON configs.osu_id = names.user_id"#,
            username
        );

        query
            .fetch_optional(self)
            .await
            .map(|row| row.and_then(|row| row.skin_url))
            .wrap_err("failed to fetch optional")
    }

    pub async fn select_twitch_id_by_osu_id(&self, user_id: u32) -> Result<Option<u64>> {
        let query = sqlx::query!(
            r#"
SELECT 
  twitch_id 
FROM 
  user_configs 
WHERE 
  osu_id = $1
  AND twitch_id IS NOT NULL"#,
            user_id as i32
        );

        let twitch_id = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?
            .and_then(|row| row.twitch_id);

        Ok(twitch_id.map(|id| id as u64))
    }

    /// Be sure wildcards (_, %) are escaped as required!
    pub async fn select_twitch_id_by_osu_name(&self, username: &str) -> Result<Option<u64>> {
        let query = sqlx::query!(
            r#"
SELECT 
  twitch_id 
FROM 
  (
    SELECT 
      twitch_id, 
      osu_id 
    FROM 
      user_configs 
    WHERE 
      twitch_id IS NOT NULL 
      AND osu_id IS NOT NULL
  ) AS configs 
  JOIN (
    SELECT 
      user_id 
    FROM 
      osu_user_names 
    WHERE 
      username ILIKE $1
  ) AS names ON configs.osu_id = names.user_id"#,
            username
        );

        let twitch_id = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?
            .and_then(|row| row.twitch_id);

        Ok(twitch_id.map(|id| id as u64))
    }

    pub async fn upsert_user_config(
        &self,
        user_id: Id<UserMarker>,
        config: &UserConfig<OsuUserId>,
    ) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO user_configs (
  discord_id, osu_id, gamemode, twitch_id, 
  score_size, show_retries, minimized_pp, 
  list_size, timezone_seconds
) 
VALUES 
  ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (discord_id) DO 
UPDATE 
SET 
  osu_id = $2, 
  gamemode = $3, 
  twitch_id = $4, 
  score_size = $5, 
  show_retries = $6, 
  minimized_pp = $7, 
  list_size = $8,
  timezone_seconds = $9"#,
            user_id.get() as i64,
            config.osu.map(|id| id as i32),
            config.mode.map(|mode| mode as i16) as Option<i16>,
            config.twitch_id.map(|id| id as i64),
            config.score_size.map(i16::from),
            config.show_retries,
            config.minimized_pp.map(i16::from),
            config.list_size.map(i16::from),
            config.timezone.map(UtcOffset::whole_seconds),
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        debug!(?user_id, "Inserted UserConfig into DB");

        Ok(())
    }

    pub async fn update_skin_url(
        &self,
        user_id: Id<UserMarker>,
        skin_url: Option<&str>,
    ) -> Result<()> {
        let query = sqlx::query!(
            r#"
UPDATE 
  user_configs 
SET 
  skin_url = $2 
WHERE 
  discord_id = $1"#,
            user_id.get() as i64,
            skin_url
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }

    pub async fn select_user_discord_id_by_osu_id(
        &self,
        user_id: u32,
    ) -> Result<Option<Id<UserMarker>>> {
        let query = sqlx::query!(
            r#"
SELECT 
  discord_id 
FROM 
  user_configs 
WHERE 
  osu_id = $1"#,
            user_id as i32
        );

        let row_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?;

        Ok(row_opt.map(|row| Id::new(row.discord_id as u64)))
    }

    pub async fn select_user_score_size(
        &self,
        user_id: Id<UserMarker>,
    ) -> Result<Option<ScoreSize>> {
        let query = sqlx::query!(
            r#"
SELECT 
  score_size 
FROM 
  user_configs 
WHERE 
  discord_id = $1"#,
            user_id.get() as i64
        );

        let score_size_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?
            .and_then(|row| row.score_size)
            .and_then(|size| ScoreSize::try_from(size).ok());

        Ok(score_size_opt)
    }

    pub async fn select_user_mode(&self, user_id: Id<UserMarker>) -> Result<Option<GameMode>> {
        let query = sqlx::query!(
            r#"
SELECT 
  gamemode 
FROM 
  user_configs 
WHERE 
  discord_id = $1"#,
            user_id.get() as i64
        );

        let row_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?;

        Ok(row_opt.and_then(|row| row.gamemode.map(|mode| GameMode::from(mode as u8))))
    }

    #[cfg(test)]
    pub async fn delete_user_config_by_discord_id(&self, user_id: Id<UserMarker>) -> Result<()> {
        let query = sqlx::query!(
            r#"
DELETE FROM 
  user_configs 
WHERE 
  discord_id = $1"#,
            user_id.get() as i64
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }
}

#[cfg(test)]
pub(in crate::impls) mod tests {
    use eyre::Result;
    use futures::Future;
    use rosu_v2::prelude::GameMode;
    use time::UtcOffset;

    use crate::{
        model::configs::{ListSize, MinimizedPp, OsuUserId, ScoreSize, UserConfig},
        tests::{database, discord_id, osu_user_id},
        Database,
    };

    fn config() -> UserConfig<OsuUserId> {
        UserConfig {
            score_size: Some(ScoreSize::AlwaysMinimized),
            list_size: Some(ListSize::Detailed),
            minimized_pp: Some(MinimizedPp::MaxPp),
            mode: Some(GameMode::Catch),
            osu: Some(osu_user_id()),
            show_retries: Some(true),
            twitch_id: None,
            timezone: Some(UtcOffset::from_whole_seconds(-7272).unwrap()),
        }
    }

    pub async fn wrap_upsert_delete<F>(psql: &Database, fut: F) -> Result<()>
    where
        F: Future<Output = Result<()>>,
    {
        let user_id = discord_id();
        let config = config();

        psql.upsert_user_config(user_id, &config).await?;
        fut.await?;
        psql.delete_user_config_by_discord_id(user_id).await?;

        Ok(())
    }

    #[tokio::test]
    async fn upsert_delete() -> Result<()> {
        let psql = database()?;

        wrap_upsert_delete(&psql, async { Ok(()) }).await
    }

    #[tokio::test]
    async fn select_by_id() -> Result<()> {
        let psql = database()?;

        let fut = async {
            let user_id = discord_id();
            let config = config();

            let db_config_opt = psql
                .select_user_config_with_osu_id_by_discord_id(user_id)
                .await?;

            let db_config = db_config_opt.unwrap();

            assert_eq!(db_config, config);

            Ok(())
        };

        wrap_upsert_delete(&psql, fut).await
    }

    #[tokio::test]
    async fn select_discord_id_by_osu_id() -> Result<()> {
        let psql = database()?;

        let user_id = discord_id();
        let config = config();

        let discord_id = psql
            .select_user_discord_id_by_osu_id(config.osu.unwrap())
            .await?;

        assert!(discord_id.is_none());

        let fut = async {
            let discord_id_opt = psql
                .select_user_discord_id_by_osu_id(config.osu.unwrap())
                .await?;
            let discord_id = discord_id_opt.unwrap();

            assert_eq!(discord_id, user_id);

            Ok(())
        };

        wrap_upsert_delete(&psql, fut).await
    }
}
