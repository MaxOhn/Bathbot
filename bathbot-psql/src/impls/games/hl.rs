use eyre::{Result, WrapErr};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{model::games::DbHlGameScore, Database};

impl Database {
    pub async fn select_higherlower_scores_by_version(
        &self,
        version: i16,
    ) -> Result<Vec<DbHlGameScore>> {
        let query = sqlx::query_as!(
            DbHlGameScore,
            r#"
SELECT 
  discord_id, 
  highscore 
FROM 
  higherlower_scores 
WHERE 
  game_version = $1"#,
            version as i16
        );

        query.fetch_all(self).await.wrap_err("failed to fetch all")
    }

    pub async fn select_higherlower_highscore(
        &self,
        user_id: Id<UserMarker>,
        version: i16,
    ) -> Result<u32> {
        let query = sqlx::query!(
            r#"
SELECT 
  highscore 
FROM 
  higherlower_scores 
WHERE 
  discord_id = $1 
  AND game_version = $2"#,
            user_id.get() as i64,
            version as i16,
        );

        let row_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?;

        Ok(row_opt.map_or(0, |row| row.highscore as u32))
    }

    /// Returns whether the score is a new highscore
    pub async fn upsert_higherlower_highscore(
        &self,
        user_id: Id<UserMarker>,
        version: i16,
        score: u32,
    ) -> Result<bool> {
        let query = sqlx::query!(
            r#"
INSERT INTO higherlower_scores (
  discord_id, game_version, highscore
) 
VALUES 
  ($1, $2, $3) ON CONFLICT (discord_id, game_version) DO 
UPDATE 
SET 
  highscore = $3 
WHERE 
  higherlower_scores.highscore < $3 RETURNING highscore"#,
            user_id.get() as i64,
            version as i16,
            score as i32,
        );

        let row_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?;

        Ok(row_opt.is_some())
    }
}
