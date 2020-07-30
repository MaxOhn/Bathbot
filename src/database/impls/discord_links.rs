use crate::{BotResult, Database};

use dashmap::DashMap;
use sqlx::Row;

impl Database {
    pub async fn add_discord_link(&self, user_id: u64, name: &str) -> BotResult<()> {
        let query = "
INSERT INTO
    discord_users
VALUES
    ($1,$2)
ON CONFLICT (discord_id) DO
    UPDATE
        SET osu_name=$2
";
        sqlx::query(query)
            .bind(user_id as i64)
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_discord_link(&self, user_id: u64) -> BotResult<()> {
        let query = format!("DELETE FROM discord_users WHERE discord_id={}", user_id);
        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_discord_links(&self) -> BotResult<DashMap<u64, String>> {
        let links = sqlx::query("SELECT * FROM discord_users")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| (row.get::<i64, _>(0) as u64, row.get(1)))
            .collect();
        Ok(links)
    }
}
