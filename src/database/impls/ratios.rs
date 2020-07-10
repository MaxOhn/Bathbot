use crate::{database::Ratios, BotResult, Database};

impl Database {
    pub async fn update_ratios(
        &self,
        name: &str,
        scores: &[i8],
        ratios: &[f32],
        misses: &[f32],
    ) -> BotResult<Option<Ratios>> {
        let query = "
    INSERT INTO
        ratio_table
    VALUES
        ($1,$2,$3,$4)
    ON CONFLICT DO
        UPDATE
            SET scores=$2,ratios=$3,misses=$4
    RETURNING *
    ";
        let old_ratios = sqlx::query_as(query)
            .bind(name)
            .bind(scores)
            .bind(ratios)
            .bind(misses)
            .fetch_optional(&self.pool)
            .await?;
        Ok(old_ratios)
    }
}
