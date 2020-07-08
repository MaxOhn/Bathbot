use crate::{database::GuildConfig, BotResult, Database};

use postgres_types::Type;

impl Database {
    pub async fn get_guild_config(&self, guild_id: u64) -> BotResult<GuildConfig> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed("SELECT config from guilds where id=$1", &[Type::INT8])
            .await?;
        if let Some(row) = client.query_one(&statement, &[&(guild_id as i64)]).await? {
            Ok(serde_json::from_value(row.get(0))?)
        } else {
            let config = GuildConfig::default();
            info!(
                "No config found for guild {}, inserting blank one",
                guild_id
            );
            let statement = client
                .prepare_typed(
                    "INSERT INTO guilds VALUES ($1, $2)",
                    &[Type::INT8, Type::JSON],
                )
                .await?;
            client
                .execute(
                    &statement,
                    &[
                        &(guild_id as i64),
                        &serde_json::to_value(&GuildConfig::default()).unwrap(),
                    ],
                )
                .await?;
            Ok(config)
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
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "UPDATE guilds SET config=$1 WHERE id=$2",
                &[Type::JSON, Type::INT8],
            )
            .await?;
        client
            .execute(&statement, &[&config, &(guild_id as i64)])
            .await?;
        Ok(())
    }

    pub async fn insert_guild(&self, guild_id: u64) -> BotResult<GuildConfig> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "INSERT INTO guilds VALUES ($1,$2) ON CONFLICT DO NOTHING",
                &[Type::INT8, Type::JSON],
            )
            .await?;
        let config = GuildConfig::default();
        client
            .execute(&statement, &[&(guild_id as i64), &config])
            .await?;
        Ok(config)
    }
}
