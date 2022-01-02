use crate::{core::AssignRoles, BotResult, Database};

use dashmap::DashMap;
use futures::stream::StreamExt;

impl Database {
    #[cold]
    pub async fn get_role_assigns(&self) -> BotResult<DashMap<(u64, u64), AssignRoles>> {
        let mut stream = sqlx::query!("SELECT * FROM role_assigns").fetch(&self.pool);
        let assigns = DashMap::with_capacity(200);

        while let Some(entry) = stream.next().await.transpose()? {
            let channel_id: i64 = entry.channel_id;
            let message_id: i64 = entry.message_id;
            let role_id: i64 = entry.role_id;

            assigns
                .entry((channel_id as u64, message_id as u64))
                .or_insert_with(AssignRoles::new)
                .push(role_id as u64);
        }

        Ok(assigns)
    }

    pub async fn add_role_assign(&self, channel: u64, message: u64, role: u64) -> BotResult<()> {
        sqlx::query!(
            "INSERT INTO role_assigns \
            VALUES ($1,$2,$3)\
            ON CONFLICT (channel_id,message_id,role_id) DO NOTHING",
            channel as i64,
            message as i64,
            role as i64,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_role_assign(
        &self,
        channel: u64,
        message: u64,
        role: u64,
    ) -> BotResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM role_assigns WHERE channel_id=$1 AND message_id=$2 AND role_id=$3",
            channel as i64,
            message as i64,
            role as i64,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
