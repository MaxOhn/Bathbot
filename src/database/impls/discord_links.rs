use crate::{BotResult, Database};

use postgres_types::Type;
use std::collections::HashMap;

impl Database {
    pub async fn add_discord_link(&self, user_id: u64, name: &str) -> BotResult<()> {
        let query = "
INSERT INTO
    discord_users
VALUES
    ($1,$2)
ON CONFLICT DO
    UPDATE
        SET osu_name=$2
";
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(query, &[Type::INT8, Type::BYTEA])
            .await?;
        client
            .execute(&statement, &[&(user_id as i64), &(name)])
            .await?;
        Ok(())
    }

    pub async fn remove_discord_link(&self, user_id: u64) -> BotResult<()> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "DELETE FROM discord_users WHERE discord_id=$1",
                &[Type::INT8],
            )
            .await?;
        client.execute(&statement, &[&(user_id as i64)]).await?;
        Ok(())
    }

    pub async fn get_discord_links(&self) -> BotResult<HashMap<u64, String>> {
        let client = self.pool.get().await?;
        let statement = client.prepare("SELECT * FROM discord_users").await?;
        let links = client
            .query(&statement, &[])
            .await?
            .into_iter()
            .map(|row| (row.get::<_, i64>(0) as u64, row.get(1)))
            .collect();
        Ok(links)
    }
}
