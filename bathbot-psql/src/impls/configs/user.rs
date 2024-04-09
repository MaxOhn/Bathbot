use eyre::{Result, WrapErr};
use futures::StreamExt;
use rosu_v2::prelude::{GameMode, Username};
use time::UtcOffset;
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    model::configs::{
        DbSkinEntry, DbUserConfig, ListSize, MinimizedPp, OsuUserId, OsuUsername, Retries,
        ScoreSize, SkinEntry, UserConfig,
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
  retries, 
  twitch_id, 
  timezone_seconds, 
  render_button, 
  legacy_scores 
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
  retries, 
  twitch_id, 
  timezone_seconds, 
  render_button, 
  legacy_scores 
FROM 
  user_configs 
WHERE 
  discord_id = $1"#,
            user_id.get() as i64,
        );

        let config_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("Failed to fetch optional")?
            .map(|row| UserConfig {
                score_size: row.score_size.map(ScoreSize::try_from).and_then(Result::ok),
                list_size: row.list_size.map(ListSize::try_from).and_then(Result::ok),
                minimized_pp: row
                    .minimized_pp
                    .map(MinimizedPp::try_from)
                    .and_then(Result::ok),
                mode: row.gamemode.map(|mode| GameMode::from(mode as u8)),
                osu: row.username.map(Username::from),
                retries: row.retries.map(Retries::try_from).and_then(Result::ok),
                twitch_id: row.twitch_id.map(|id| id as u64),
                timezone: row
                    .timezone_seconds
                    .map(UtcOffset::from_whole_seconds)
                    .map(Result::unwrap),
                render_button: row.render_button,
                legacy_scores: row.legacy_scores,
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

    pub async fn select_all_skins(&self) -> Result<Vec<SkinEntry>> {
        let query = sqlx::query_as!(
            DbSkinEntry,
            r#"
SELECT 
  osu.user_id, 
  username, 
  skin_url 
FROM 
  (
    SELECT DISTINCT ON (osu_id) 
      skin_url, 
      osu_id 
    FROM 
      user_configs 
    WHERE 
      skin_url IS NOT NULL 
      AND osu_id IS NOT NULL
  ) AS configs 
  JOIN osu_user_names AS osu ON configs.osu_id = osu.user_id 
  JOIN (
    SELECT 
      user_id, 
      MIN(global_rank) AS global_rank 
    FROM 
      osu_user_mode_stats 
    WHERE 
      global_rank > 0 
    GROUP BY 
      user_id
  ) AS stats ON osu.user_id = stats.user_id 
ORDER BY 
  global_rank"#
        );

        let mut rows = query.fetch(self);
        let mut entries = Vec::with_capacity(64);

        while let Some(entry_res) = rows.next().await {
            let entry = entry_res.wrap_err("Failed to get next")?;
            entries.push(entry.into());
        }

        Ok(entries)
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
        let UserConfig {
            score_size,
            list_size,
            minimized_pp,
            mode,
            osu,
            retries,
            twitch_id,
            timezone,
            render_button,
            legacy_scores,
        } = config;

        let query = sqlx::query!(
            r#"
INSERT INTO user_configs (
  discord_id, osu_id, gamemode, twitch_id, 
  score_size, retries, minimized_pp, 
  list_size, timezone_seconds, render_button, 
  legacy_scores
) 
VALUES 
  ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT (discord_id) DO 
UPDATE 
SET 
  osu_id = $2, 
  gamemode = $3, 
  twitch_id = $4, 
  score_size = $5, 
  retries = $6, 
  minimized_pp = $7, 
  list_size = $8, 
  timezone_seconds = $9, 
  render_button = $10, 
  legacy_scores = $11"#,
            user_id.get() as i64,
            osu.map(|id| id as i32),
            mode.map(|mode| mode as i16) as Option<i16>,
            twitch_id.map(|id| id as i64),
            score_size.map(i16::from),
            retries.map(i16::from),
            minimized_pp.map(i16::from),
            list_size.map(i16::from),
            timezone.map(UtcOffset::whole_seconds),
            *render_button,
            *legacy_scores,
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        debug!(user_id = user_id.get(), "Inserted UserConfig into DB");

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
}
