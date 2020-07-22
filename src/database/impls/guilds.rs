use crate::{database::GuildConfig, BotResult, Database};

use dashmap::DashMap;
use sqlx::{types::Json, FromRow, Row};
use twilight::model::id::GuildId;

impl Database {
    pub async fn get_guild_config(&self, guild_id: u64) -> BotResult<GuildConfig> {
        let query = format!("SELECT config from guilds where guild_id={}", guild_id);
        match sqlx::query_as::<_, GuildConfig>(&query)
            .fetch_optional(&self.pool)
            .await?
        {
            Some(config) => Ok(config),
            None => {
                info!(
                    "No config found for guild {}, inserting blank one",
                    guild_id
                );
                self.insert_guild(guild_id).await
            }
        }
    }

    pub async fn get_guilds(&self) -> BotResult<DashMap<GuildId, GuildConfig>> {
        let guilds = sqlx::query("SELECT * FROM guilds")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| {
                let id: i64 = row.get(0);
                let config = GuildConfig::from_row(&row).unwrap();
                (GuildId(id as u64), config)
            })
            .collect();
        Ok(guilds)
    }

    pub async fn set_guild_config(&self, guild_id: u64, config: &GuildConfig) -> BotResult<()> {
        sqlx::query("UPDATE guilds SET config=$1 WHERE guild_id=$2")
            .bind(Json(config))
            .bind(guild_id as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_guild(&self, guild_id: u64) -> BotResult<GuildConfig> {
        let query = "
INSERT INTO
    guilds
VALUES
    ($1,$2)
ON CONFLICT DO
    NOTHING
RETURNING
    config";
        sqlx::query(query)
            .bind(guild_id as i64)
            .bind(Json(GuildConfig::default()))
            .execute(&self.pool)
            .await?;
        Ok(GuildConfig::default())
    }
}
