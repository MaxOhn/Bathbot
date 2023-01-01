use eyre::{Result, WrapErr};
use rosu_v2::prelude::GameMode;
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    model::games::{DbBgGameScore, DbMapTagEntry, DbMapTagsParams},
    Database,
};

impl Database {
    pub async fn increment_bggame_score(&self, user_id: Id<UserMarker>, amount: i32) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO bggame_scores (discord_id, score) 
VALUES 
  ($1, $2) ON CONFLICT (discord_id) DO 
UPDATE 
SET 
  score = bggame_scores.score + $2"#,
            user_id.get() as i64,
            amount
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }

    pub async fn select_bggame_scores(&self) -> Result<Vec<DbBgGameScore>> {
        let query = sqlx::query_as!(
            DbBgGameScore,
            r#"
SELECT 
  discord_id, 
  score 
FROM 
  bggame_scores"#
        );

        query.fetch_all(self).await.wrap_err("failed to fetch all")
    }

    pub async fn upsert_map_tag(
        &self,
        mapset_id: u32,
        filename: &str,
        mode: GameMode,
    ) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO map_tags (
  mapset_id, image_filename, gamemode
) 
VALUES 
  ($1, $2, $3) ON CONFLICT (mapset_id) DO 
UPDATE 
SET 
  image_filename = $2"#,
            mapset_id as i32,
            filename,
            mode as i16
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }

    pub async fn select_map_tags(&self, params: DbMapTagsParams) -> Result<Vec<DbMapTagEntry>> {
        let query = params.into_query();

        sqlx::query_as(&query)
            .fetch_all(self)
            .await
            .wrap_err("failed to fetch all")
    }
}
