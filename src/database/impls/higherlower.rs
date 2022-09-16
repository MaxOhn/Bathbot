use eyre::Result;
use tokio_stream::StreamExt;

use crate::{games::hl::HlVersion, Database};

impl Database {
    pub async fn get_higherlower_scores(&self, version: HlVersion) -> Result<Vec<(u64, u32)>> {
        let query = sqlx::query!(
            "SELECT discord_id,highscore \
            FROM higherlower_scores \
            WHERE version=$1",
            version as i16
        );

        let scores = query
            .fetch(&self.pool)
            .map(|res| res.map(|entry| (entry.discord_id as u64, entry.highscore as u32)))
            .collect::<Result<_, _>>()
            .await?;

        Ok(scores)
    }

    pub async fn get_higherlower_highscore(&self, user_id: u64, version: HlVersion) -> Result<u32> {
        let query = sqlx::query!(
            "SELECT highscore FROM higherlower_scores \
            WHERE discord_id=$1 AND version=$2",
            user_id as i64,
            version as i16,
        );

        match query.fetch_optional(&self.pool).await?.map(|e| e.highscore) {
            Some(score) => Ok(score as u32),
            None => Ok(0),
        }
    }

    /// Caller must provide proper highscore value retrieved from [`get_higherlower_highscore`](Database::get_higherlower_highscore)
    pub async fn upsert_higherlower_highscore(
        &self,
        user_id: u64,
        version: HlVersion,
        score: u32,
        highscore: u32,
    ) -> Result<bool> {
        if score <= highscore {
            return Ok(false);
        }

        sqlx::query!(
            "INSERT INTO higherlower_scores \
                VALUES ($1, $2, $3) ON CONFLICT (discord_id, version) DO \
                UPDATE \
                SET highscore=$3",
            user_id as i64,
            version as i16,
            score as i32
        )
        .execute(&self.pool)
        .await?;

        Ok(true)
    }
}
