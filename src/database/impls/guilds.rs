use crate::{database::GuildConfig, BotResult, Database};

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

    // pub async fn get_guilds(&self) -> BotResult<HashMap<GuildId, Guild>> {
    //     let guilds = sqlx::query_as::<_, Guild>("SELECT * FROM guilds")
    //         .fetch(&self.pool)
    //         .filter_map(|result| match result {
    //             Ok(g) => Some((g.guild_id, g)),
    //             Err(why) => {
    //                 warn!("Error while getting guilds from DB: {}", why);
    //                 None
    //             }
    //         })
    //         .collect::<Vec<_>>()
    //         .await
    //         .into_iter()
    //         .collect();
    //     Ok(guilds)
    // }

    pub async fn set_guild_config(&self, guild_id: u64, config: GuildConfig) -> BotResult<()> {
        sqlx::query("UPDATE guilds SET config=$1 WHERE id=$2")
            .bind(serde_json::to_string(&config)?)
            .bind(guild_id as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_guild(&self, guild_id: u64) -> BotResult<GuildConfig> {
        let config = GuildConfig::default();
        sqlx::query("INSERT INTO guild VALUES ($1,$2) ON CONFLICT DO NOTHING")
            .bind(guild_id as i64)
            .bind(serde_json::to_string(&config).unwrap())
            .execute(&self.pool)
            .await?;
        Ok(config)
    }
}
