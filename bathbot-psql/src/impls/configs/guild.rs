use std::{collections::HashMap, hash::BuildHasher};

use eyre::{Result, WrapErr};
use futures::StreamExt;
use twilight_model::id::{marker::GuildMarker, Id};

use crate::{
    model::configs::{DbGuildConfig, GuildConfig},
    Database,
};

impl Database {
    pub async fn select_guild_configs<S>(&self) -> Result<HashMap<Id<GuildMarker>, GuildConfig, S>>
    where
        S: Default + BuildHasher,
    {
        info!("Fetching guild configs...");

        let query = sqlx::query_as!(
            DbGuildConfig,
            r#"
SELECT 
  guild_id,
  authorities,
  prefixes,
  allow_songs,
  score_size,
  retries,
  osu_track_limit,
  minimized_pp,
  list_size, 
  render_button, 
  allow_custom_skins, 
  hide_medal_solution 
FROM 
  guild_configs"#
        );

        let mut rows = query.fetch(self);
        let mut configs = HashMap::with_capacity_and_hasher(30_000, S::default());

        while let Some(row_res) = rows.next().await {
            let row = row_res.wrap_err("failed to get next")?;
            let guild_id = Id::new(row.guild_id as u64);
            configs.insert(guild_id, row.into());
        }

        Ok(configs)
    }

    pub async fn upsert_guild_config(
        &self,
        guild_id: Id<GuildMarker>,
        config: &GuildConfig,
    ) -> Result<()> {
        let GuildConfig {
            authorities,
            score_size,
            list_size,
            minimized_pp,
            prefixes,
            retries,
            track_limit,
            allow_songs,
            render_button,
            allow_custom_skins,
            hide_medal_solution,
        } = config;

        let authorities =
            rkyv::to_bytes::<_, 1>(authorities).wrap_err("failed to serialize authorities")?;

        let prefixes =
            rkyv::to_bytes::<_, 32>(prefixes).wrap_err("failed to serialize prefixes")?;

        let query = sqlx::query!(
            r#"
INSERT INTO guild_configs (
  guild_id, authorities, prefixes, allow_songs, 
  score_size, retries, osu_track_limit, 
  minimized_pp, list_size, render_button, 
  allow_custom_skins, hide_medal_solution
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11, $12
  ) ON CONFLICT (guild_id) DO 
UPDATE 
SET 
  authorities = $2, 
  prefixes = $3, 
  allow_songs = $4, 
  score_size = $5, 
  retries = $6, 
  osu_track_limit = $7, 
  minimized_pp = $8, 
  list_size = $9, 
  render_button = $10, 
  allow_custom_skins = $11, 
  hide_medal_solution = $12"#,
            guild_id.get() as i64,
            &authorities as &[u8],
            &prefixes as &[u8],
            *allow_songs,
            score_size.map(i16::from),
            retries.map(i16::from),
            track_limit.map(|limit| limit as i16),
            minimized_pp.map(i16::from),
            list_size.map(i16::from),
            *render_button,
            *allow_custom_skins,
            hide_medal_solution.map(i16::from),
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        debug!(guild_id = guild_id.get(), "Inserted GuildConfig into DB");

        Ok(())
    }

    #[cfg(test)]
    pub async fn delete_guild_config_by_discord_id(&self, guild_id: Id<GuildMarker>) -> Result<()> {
        let query = sqlx::query!(
            r#"
DELETE FROM 
  guild_configs 
WHERE 
  guild_id = $1"#,
            guild_id.get() as i64
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

    use crate::{
        model::configs::{
            Authorities, GuildConfig, HideSolutions, ListSize, MinimizedPp, Prefixes, Retries,
            ScoreSize,
        },
        tests::{database, discord_id},
        Database,
    };

    fn config() -> GuildConfig {
        let mut authorities = Authorities::default();
        authorities.push(discord_id());

        let mut prefixes = Prefixes::default();
        prefixes.try_push("!!".into()).unwrap();
        prefixes.try_push("HOLY SHIT LOOK AT THIS ".into()).unwrap();
        prefixes
            .try_push(
                "jarvis if you would be so kind, could you please show me the command ".into(),
            )
            .unwrap();

        GuildConfig {
            authorities,
            score_size: Some(ScoreSize::AlwaysMinimized),
            list_size: Some(ListSize::Detailed),
            minimized_pp: Some(MinimizedPp::MaxPp),
            prefixes,
            retries: Some(Retries::IgnoreMods),
            track_limit: Some(42),
            allow_songs: Some(true),
            render_button: Some(true),
            allow_custom_skins: None,
            hide_medal_solution: Some(HideSolutions::HideHushHush),
        }
    }

    pub async fn wrap_upsert_delete<F>(psql: &Database, fut: F) -> Result<()>
    where
        F: Future<Output = Result<()>>,
    {
        let guild_id = discord_id();
        let config = config();

        psql.upsert_guild_config(guild_id, &config).await?;
        fut.await?;
        psql.delete_guild_config_by_discord_id(guild_id).await?;

        Ok(())
    }

    #[tokio::test]
    async fn upsert_delete() -> Result<()> {
        let psql = database()?;

        wrap_upsert_delete(&psql, async { Ok(()) }).await
    }
}
