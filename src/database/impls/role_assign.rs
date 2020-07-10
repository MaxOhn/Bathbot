use crate::{BotResult, Database};

use postgres_types::Type;
use std::collections::HashMap;

impl Database {
    pub async fn get_role_assigns(&self) -> BotResult<HashMap<(u64, u64), u64>> {
        let client = self.pool.get().await?;
        let statement = client.prepare("SELECT * FROM role_assign").await?;
        let assigns = client
            .query(&statement, &[])
            .await?
            .into_iter()
            .map(|row| {
                let channel: i64 = row.get(0);
                let msg: i64 = row.get(1);
                let role: i64 = row.get(2);
                ((channel as u64, msg as u64), role as u64)
            })
            .collect();
        Ok(assigns)
    }

    pub async fn add_role_assign(&self, channel: u64, message: u64, role: u64) -> BotResult<()> {
        let query = "
INSERT INTO
    role_assign
VALUES
    ($1,$2,$3)
ON CONFLICT DO
    UPDATE
        SET role=$3
";
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(query, &[Type::INT8, Type::INT8, Type::INT8])
            .await?;
        client
            .execute(
                &statement,
                &[&(channel as i64), &(message as i64), &(role as i64)],
            )
            .await?;
        Ok(())
    }
}
