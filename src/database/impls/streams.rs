use crate::{database::util::CustomSQL, BotResult, Database};

use postgres_types::Type;
use std::collections::{HashMap, HashSet};

impl Database {
    pub async fn add_twitch_user(&self, user_id: u64, name: &str) -> BotResult<()> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "INSERT INTO twitch_users VALUES ($1,$2)",
                &[Type::INT8, Type::BYTEA],
            )
            .await?;
        client
            .execute(&statement, &[&(user_id as i64), &(name)])
            .await?;
        Ok(())
    }

    pub async fn add_stream_track(&self, channel: u64, user: u64) -> BotResult<()> {
        let client = self.pool.get().await?;
        let statement = client
            .prepare_typed(
                "INSERT INTO stream_tracks VALUES ($1,$2)",
                &[Type::INT8, Type::INT8],
            )
            .await?;
        client
            .execute(&statement, &[&(channel as i64), &(user as i64)])
            .await?;
        Ok(())
    }

    pub async fn get_twitch_users(&self, user_ids: &[u64]) -> BotResult<HashMap<String, u64>> {
        let query = String::from("SELECT * FROM twitch_users WHERE user_id IN").in_clause(user_ids);
        let client = self.pool.get().await?;
        let statement = client.prepare(&query).await?;
        let users = client
            .query(&statement, &[])
            .await?
            .into_iter()
            .map(|row| {
                let user_id: i64 = row.get(0);
                (row.get(1), user_id as u64)
            })
            .collect();
        Ok(users)
    }

    pub async fn get_stream_tracks(&self) -> BotResult<HashSet<(u64, u64)>> {
        let client = self.pool.get().await?;
        let statement = client.prepare("SELECT * FROM stream_tracks").await?;
        let tracks = client
            .query(&statement, &[])
            .await?
            .into_iter()
            .map(|row| {
                let channel: i64 = row.get(0);
                let user: i64 = row.get(1);
                (channel as u64, user as u64)
            })
            .collect();
        Ok(tracks)
    }

    pub async fn remove_stream_track(&self, channel: u64, user: u64) -> BotResult<()> {
        let client = self.pool.get().await?;
        let query = "
DELETE FROM
    stream_tracks
WHERE
    channel_id=$1
    AND user_id=$2
";
        let statement = client
            .prepare_typed(query, &[Type::INT8, Type::INT8])
            .await?;
        client
            .execute(&statement, &[&(channel as i64), &(user as i64)])
            .await?;
        Ok(())
    }
}
