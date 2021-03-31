use crate::{database::GuildConfig, BotResult, Database};

use dashmap::DashMap;
use futures::stream::StreamExt;
use twilight_model::id::GuildId;

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

    pub async fn insert_guilds(&self, configs: &DashMap<GuildId, GuildConfig>) -> BotResult<usize> {
        let mut counter = 0;
        let mut result = Ok(());

        for guard in configs.iter().filter(|guard| guard.value().modified) {
            result = sqlx::query!(
                "INSERT INTO guild_configs VALUES ($1,$2) 
                ON CONFLICT (guild_id) DO UPDATE SET config=$2",
                guard.key().0 as i64,
                serde_json::to_value(guard.value())?
            )
            .execute(&self.pool)
            .await
            .and(result);

            counter += 1;
        }

        result?;

        Ok(counter)
    }
}
