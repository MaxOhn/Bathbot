use crate::{BotResult, Database};

impl Database {
    pub async fn get_higherlower_highscore(&self, user_id: u64, mode: u8) -> BotResult<u32> {
        let query = sqlx::query!(
            "SELECT highscore FROM higherlower_scores \
            WHERE discord_id=$1 AND mode=$2",
            user_id as i64,
            mode as i16,
        );

        match query.fetch_optional(&self.pool).await?.map(|e| e.highscore) {
            Some(score) => Ok(score as u32),
            None => Ok(0),
        }
    }

    pub async fn upsert_higherlower_highscore(
        &self,
        user_id: u64,
        mode: u8,
        score: u32,
        highscore: u32,
    ) -> BotResult<bool> {
        //! Caller should provide proper highscore value retrieved from get_higherlower_highscore
        if score > highscore {
            sqlx::query!(
                "INSERT INTO higherlower_scores \
                    VALUES ($1, $2, $3) ON CONFLICT (discord_id, mode) DO \
                    UPDATE \
                    SET highscore=$3",
                user_id as i64,
                mode as i16,
                score as i32
            )
            .execute(&self.pool)
            .await?;

            return Ok(true);
        }

        Ok(false)
    }
}
