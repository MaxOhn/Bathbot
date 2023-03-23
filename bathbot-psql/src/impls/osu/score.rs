use eyre::{Result, WrapErr};
use rosu_v2::prelude::{Score, ScoreStatistics};

use crate::database::Database;

impl Database {
    pub async fn insert_scores(&self, scores: &[Score]) -> Result<()> {
        let mut tx = self.begin().await.wrap_err("failed to begin transaction")?;

        for score in scores {
            let Score {
                accuracy: _,
                ended_at,
                grade,
                map_id,
                max_combo,
                map: _, // updated through checksum-missmatch
                mapset,
                mode,
                mods,
                passed: _,
                perfect,
                pp,
                rank_country: _,
                rank_global: _,
                replay: _,
                score,
                score_id,
                statistics:
                    ScoreStatistics {
                        count_geki,
                        count_300,
                        count_katu,
                        count_100,
                        count_50,
                        count_miss,
                    },
                user: _,
                user_id,
                weight: _,
            } = score;

            let Some(score_id) = score_id else { continue };

            let query = sqlx::query!(
                r#"
INSERT INTO osu_scores (
  score_id, user_id, map_id, gamemode, 
  mods, score, maxcombo, grade, count50, 
  count100, count300, countmiss, countgeki, 
  countkatu, perfect, ended_at
) 
VALUES 
  (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 
    $11, $12, $13, $14, $15, $16
  ) ON CONFLICT (score_id) DO NOTHING"#,
                *score_id as i64,
                *user_id as i32,
                *map_id as i32,
                *mode as i16,
                mods.bits() as i32,
                *score as i64,
                *max_combo as i32,
                *grade as i16,
                *count_50 as i16,
                *count_100 as i16,
                *count_300 as i16,
                *count_miss as i16,
                *count_geki as i16,
                *count_katu as i16,
                perfect,
                ended_at,
            );

            query
                .execute(&mut tx)
                .await
                .wrap_err("failed to execute score query")?;

            if let Some(pp) = pp {
                let query = sqlx::query!(
                    r#"
INSERT INTO osu_scores_performance (score_id, pp) 
VALUES 
  ($1, $2) ON CONFLICT (score_id) DO NOTHING"#,
                    *score_id as i64,
                    *pp as f64,
                );

                query
                    .execute(&mut tx)
                    .await
                    .wrap_err("failed to execute pp query")?;
            }

            if let Some(mapset) = mapset {
                Self::update_beatmapset_compact(&mut tx, mapset)
                    .await
                    .wrap_err("failed to update mapset")?;
            }
        }

        tx.commit().await.wrap_err("failed to commit transaction")?;

        Ok(())
    }

    pub async fn update_beatmapsets_compact(&self, scores: &[Score]) -> Result<()> {
        let mut tx = self.begin().await.wrap_err("failed to begin transaction")?;

        for score in scores {
            if let Some(ref mapset) = score.mapset {
                Self::update_beatmapset_compact(&mut tx, mapset).await?;
            }
        }

        tx.commit().await.wrap_err("failed to commit transaction")?;

        Ok(())
    }
}
