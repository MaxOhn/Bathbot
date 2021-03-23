use crate::{BotResult, Database, Name};

use dashmap::DashMap;
use futures::stream::StreamExt;

struct LinkEntry {
    discord_id: i64,
    osu_name: String,
}

impl Database {
    pub async fn add_discord_link(&self, user_id: u64, name: &str) -> BotResult<()> {
        sqlx::query!(
            "INSERT INTO discord_user_links VALUES ($1,$2) 
            ON CONFLICT (discord_id) DO UPDATE SET osu_name=$2",
            user_id as i64,
            name,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_discord_link(&self, user_id: u64) -> BotResult<()> {
        sqlx::query!(
            "DELETE FROM discord_user_links WHERE discord_id=$1",
            user_id as i64,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[cold]
    pub async fn get_discord_links(&self) -> BotResult<DashMap<u64, Name>> {
        let mut stream =
            sqlx::query_as!(LinkEntry, "SELECT * FROM discord_user_links").fetch(&self.pool);

        let links = DashMap::with_capacity(10_000);

        while let Some(link) = stream.next().await.transpose()? {
            links.insert(link.discord_id as u64, link.osu_name.into());
        }

        Ok(links)
    }
}
