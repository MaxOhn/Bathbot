use eyre::{Result, WrapErr};
use rosu_v2::prelude::Score;

use crate::database::Database;

impl Database {
    pub async fn insert_scores_mapsets(&self, scores: &[Score]) -> Result<()> {
        let mut tx = self.begin().await.wrap_err("Failed to begin transaction")?;

        for chunk in scores.chunks(100) {
            let mapset_iter = chunk.iter().filter_map(|score| score.mapset.as_deref());

            Self::update_beatmapsets(&mut *tx, mapset_iter, chunk.len())
                .await
                .wrap_err("Failed to update mapset")?
        }

        tx.commit().await.wrap_err("Failed to commit transaction")?;

        Ok(())
    }
}
