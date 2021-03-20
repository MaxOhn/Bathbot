use crate::{BotResult, Database};

use dashmap::DashMap;
use futures::stream::StreamExt;

impl Database {
    #[cold]
    pub async fn get_role_assigns(&self) -> BotResult<DashMap<(u64, u64), u64>> {
        let mut stream = sqlx::query!("SELECT * FROM role_assigns").fetch(&self.pool);
        let assigns = DashMap::with_capacity(200);

        while let Some(entry) = stream.next().await.transpose()? {
            let channel_id: i64 = entry.channel_id;
            let message_id: i64 = entry.message_id;
            let role_id: i64 = entry.role_id;

            assigns.insert((channel_id as u64, message_id as u64), role_id as u64);
        }

        Ok(assigns)
    }

    pub async fn add_role_assign(&self, channel: u64, message: u64, role: u64) -> BotResult<()> {
        let query = format!(
            "INSERT INTO role_assigns VALUES ({},{},{role}) ON CONFLICT (channel_id,message_id,role_id) DO UPDATE SET role_id={role}",
            channel,
            message,
            role = role
        );

        sqlx::query(&query).execute(&self.pool).await?;

        Ok(())
    }
}
