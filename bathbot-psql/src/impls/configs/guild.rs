use std::{collections::HashMap, hash::BuildHasher};

use eyre::{Report, Result, WrapErr};
use futures::StreamExt;
use rkyv::{rancor::BoxedError, ser::Serializer};
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
  retries,
  osu_track_limit,
  list_size, 
  render_button, 
  allow_custom_skins, 
  hide_medal_solution, 
  score_data 
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
            list_size,
            prefixes,
            retries,
            track_limit,
            allow_songs,
            render_button,
            allow_custom_skins,
            hide_medal_solution,
            score_data,
        } = config;

        let (authorities, prefixes) = rkyv::util::with_arena(|arena| {
            let mut authorities_writer = Vec::new();
            let mut prefixes_writer = Vec::new();
            let mut serializer = Serializer::new(&mut authorities_writer, arena.acquire(), ());
            rkyv::api::serialize_using::<_, BoxedError>(authorities, &mut serializer)
                .wrap_err("Failed to serialize authorities")?;

            serializer.writer = &mut prefixes_writer;
            rkyv::api::serialize_using::<_, BoxedError>(prefixes, &mut serializer)
                .wrap_err("Failed to serialize prefixes")?;

            Ok::<_, Report>((authorities_writer, prefixes_writer))
        })?;

        let query = sqlx::query!(
            r#"
INSERT INTO guild_configs (
  guild_id, authorities, prefixes, allow_songs, 
  retries, osu_track_limit, list_size, 
  render_button, allow_custom_skins, 
  hide_medal_solution, score_data
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11
  ) ON CONFLICT (guild_id) DO 
UPDATE 
SET 
  authorities = $2, 
  prefixes = $3, 
  allow_songs = $4, 
  retries = $5, 
  osu_track_limit = $6, 
  list_size = $7, 
  render_button = $8, 
  allow_custom_skins = $9, 
  hide_medal_solution = $10, 
  score_data = $11"#,
            guild_id.get() as i64,
            &authorities as &[u8],
            &prefixes as &[u8],
            *allow_songs,
            retries.map(i16::from),
            track_limit.map(|limit| limit as i16),
            list_size.map(i16::from),
            *render_button,
            *allow_custom_skins,
            hide_medal_solution.map(i16::from),
            score_data.map(i16::from),
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        debug!(guild_id = guild_id.get(), "Inserted GuildConfig into DB");

        Ok(())
    }
}
