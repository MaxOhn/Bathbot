use crate::{BotResult, Database};

use dashmap::DashMap;
use sqlx::Row;

impl Database {
    #[cold]
    pub async fn get_role_assigns(&self) -> BotResult<DashMap<(u64, u64), u64>> {
        let assigns = sqlx::query("SELECT * FROM role_assign")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| {
                (
                    (row.get::<i64, _>(0) as u64, row.get::<i64, _>(1) as u64),
                    row.get::<i64, _>(2) as u64,
                )
            })
            .collect();

        Ok(assigns)
    }

    pub async fn add_role_assign(&self, channel: u64, message: u64, role: u64) -> BotResult<()> {
        let query = format!(
            "INSERT INTO role_assign VALUES ({},{},{role}) ON CONFLICT (channel, message, role) DO UPDATE SET role={role}",
            channel,
            message,
            role = role
        );

        sqlx::query(&query).execute(&self.pool).await?;

        Ok(())
    }
}
