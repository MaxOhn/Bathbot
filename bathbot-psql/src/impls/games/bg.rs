use std::mem;

use bathbot_model::BgGameScore;
use eyre::{Result, WrapErr};
use rosu_v2::prelude::GameMode;

use crate::{
    model::games::{DbBgGameScore, DbMapTagEntry, DbMapTagsParams},
    Database,
};

impl Database {
    pub async fn increment_bggame_scores(&self, user_ids: &[i64], amounts: &[i32]) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO bggame_scores (discord_id, score) 
SELECT
  *
FROM
  UNNEST($1::INT8[], $2::INT4[]) ON CONFLICT (discord_id) DO 
UPDATE 
SET 
  score = bggame_scores.score + excluded.score"#,
            user_ids,
            amounts,
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }

    pub async fn select_bggame_scores(&self) -> Result<Vec<BgGameScore>> {
        let query = sqlx::query_as!(
            DbBgGameScore,
            r#"
SELECT 
  discord_id, 
  score 
FROM 
  bggame_scores"#
        );

        let scores = query
            .fetch_all(self)
            .await
            .wrap_err("failed to fetch all")?;

        // SAFETY: the two types have the exact same structure
        Ok(unsafe { mem::transmute::<Vec<DbBgGameScore>, Vec<BgGameScore>>(scores) })
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
