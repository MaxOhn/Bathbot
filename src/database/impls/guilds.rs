use crate::{database::GuildConfig, BotResult, Database};

use dashmap::DashMap;
use sqlx::{types::Json, FromRow, Row};
use twilight_model::id::GuildId;

impl Database {
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

    pub async fn insert_guilds(&self, configs: &DashMap<GuildId, GuildConfig>) -> BotResult<usize> {
        let mut counter = 0;
        let mut result = Ok(());

        for guard in configs.iter().filter(|guard| guard.value().modified) {
            let query = format!(
                "INSERT INTO guilds VALUES ({},$1) ON CONFLICT (guild_id) DO UPDATE SET config=$1",
                guard.key()
            );

            result = sqlx::query(&query)
                .bind(Json(guard.value()))
                .execute(&self.pool)
                .await
                .and(result);

            counter += 1;
        }

        result?;

        Ok(counter)
    }
}
