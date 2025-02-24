use std::{collections::HashMap, hash::BuildHasher};

use eyre::{Report, Result, WrapErr};
use futures::StreamExt;
use rkyv::{rancor::BoxedError, ser::Serializer};
use sqlx::types::Json;
use twilight_model::id::{Id, marker::GuildMarker};

use crate::{
    Database,
    model::configs::{DbGuildConfig, GuildConfig},
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
  retries,
  list_size, 
  render_button, 
  allow_custom_skins, 
  hide_medal_solution, 
  score_data 
FROM 
  guild_configs"#
        );

        let mut rows = query.fetch(self);
        let mut configs = HashMap::with_capacity_and_hasher(50_000, S::default());

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
            list_size,
            prefixes,
            retries,
            allow_songs,
            render_button,
            allow_custom_skins,
            hide_medal_solution,
            score_data,
        } = config;

        let authorities = rkyv::util::with_arena(|arena| {
            let mut writer = Vec::new();
            let mut serializer = Serializer::new(&mut writer, arena.acquire(), ());
            rkyv::api::serialize_using::<_, BoxedError>(authorities, &mut serializer)
                .wrap_err("Failed to serialize authorities")?;

            Ok::<_, Report>(writer)
        })?;

        let query = sqlx::query!(
            r#"
INSERT INTO guild_configs (
  guild_id, authorities, prefixes, allow_songs, 
  retries, list_size, 
  render_button, allow_custom_skins, 
  hide_medal_solution, score_data
) 
VALUES 
  ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
ON CONFLICT
  (guild_id)
DO 
  UPDATE 
SET 
  authorities = $2, 
  prefixes = $3, 
  allow_songs = $4, 
  retries = $5, 
  list_size = $6, 
  render_button = $7, 
  allow_custom_skins = $8, 
  hide_medal_solution = $9, 
  score_data = $10"#,
            guild_id.get() as i64,
            &authorities as &[u8],
            Json(prefixes) as _,
            *allow_songs,
            retries.map(i16::from),
            list_size.map(i16::from),
            *render_button,
            *allow_custom_skins,
            hide_medal_solution.map(i16::from),
            score_data.map(i16::from),
        );

        query
            .execute(self)
            .await
            .wrap_err("Failed to execute query")?;

        debug!(guild_id = guild_id.get(), "Inserted GuildConfig into DB");

        Ok(())
    }
}
