use crate::{database::Ratios, BotResult, Database};

impl Database {
    pub async fn update_ratios(
        &self,
        name: &str,
        scores: &[i8],
        ratios: &[f32],
        misses: &[f32],
    ) -> BotResult<Option<Ratios>> {
        let get_query = "SELECT * FROM ratio_table WHERE name=$1 LIMIT 1";

        let old_ratios: Option<Ratios> = sqlx::query_as(get_query)
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        let update_query = "INSERT INTO ratio_table VALUES ($1,$2,$3,$4) ON CONFLICT (name) DO UPDATE SET scores=$2,ratios=$3,misses=$4";

        sqlx::query(update_query)
            .bind(name)
            .bind(scores)
            .bind(ratios)
            .bind(misses)
            .execute(&self.pool)
            .await?;

        Ok(old_ratios)
    }
}
