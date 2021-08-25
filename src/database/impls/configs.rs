use crate::{
    database::{GuildConfig, UserConfig},
    BotResult, Database,
};

use dashmap::DashMap;
use futures::stream::StreamExt;
use sqlx::Error;
use twilight_model::id::{GuildId, UserId};

impl Database {
    #[cold]
    pub async fn get_guilds(&self) -> BotResult<DashMap<GuildId, GuildConfig>> {
        let mut stream = sqlx::query!("SELECT * FROM guild_configs").fetch(&self.pool);
        let guilds = DashMap::with_capacity(3000);

        while let Some(entry) = stream.next().await.transpose()? {
            let guild_id: i64 = entry.guild_id;
            let config = serde_json::from_value(entry.config)?;

            guilds.insert(GuildId(guild_id as u64), config);
        }

        Ok(guilds)
    }

    pub async fn insert_guild_config(
        &self,
        guild_id: GuildId,
        config: &GuildConfig,
    ) -> BotResult<()> {
        let query = sqlx::query!(
            "INSERT INTO guild_configs VALUES ($1,$2) 
            ON CONFLICT (guild_id) DO UPDATE SET config=$2",
            guild_id.0 as i64,
            serde_json::to_value(config)?
        );

        query.execute(&self.pool).await?;
        info!("Inserted GuildConfig for guild {} into DB", guild_id);

        Ok(())
    }

    pub async fn get_user_config(&self, user_id: UserId) -> BotResult<Option<UserConfig>> {
        let query = sqlx::query!(
            "SELECT config FROM user_configs WHERE user_id=$1",
            user_id.0 as i64
        );

        match query.fetch_one(&self.pool).await {
            Ok(entry) => Ok(serde_json::from_value(entry.config)?).map(Some),
            Err(Error::RowNotFound) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn insert_user_config(&self, user_id: UserId, config: &UserConfig) -> BotResult<()> {
        let query = sqlx::query!(
            "INSERT INTO user_configs VALUES ($1,$2) 
            ON CONFLICT (user_id) DO UPDATE SET config=$2",
            user_id.0 as i64,
            serde_json::to_value(config)?
        );

        query.execute(&self.pool).await?;
        debug!("Inserted UserConfig for user {} into DB", user_id);

        Ok(())
    }
}
