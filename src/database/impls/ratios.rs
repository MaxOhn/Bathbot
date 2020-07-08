use crate::{database::Ratios, BotResult, Database};

use postgres_types::Type;

impl Database {
    pub async fn update_ratios(
        &self,
        name: &str,
        scores: &[i8],
        ratios: &[f32],
        misses: &[f32],
    ) -> BotResult<Option<Ratios>> {
        let select_query = "
SELECT
    scores,ratios,misses
FROM
    ratio_table
WHERE
    name=$1
";
        let upsert_query = "
INSERT INTO
    ratio_table
VALUES
    ($1,$2,$3,$4)
ON CONFLICT DO
    UPDATE
        SET scores=$2,ratios=$3,misses=$4
";
        let client = self.pool.get().await?;
        let txn = client.transaction().await?;
        let select_stmnt = txn
            .prepare_typed(
                select_query,
                &[
                    Type::BYTEA,
                    Type::CHAR_ARRAY,
                    Type::FLOAT4_ARRAY,
                    Type::FLOAT4_ARRAY,
                ],
            )
            .await?;
        let row = txn
            .query_opt(select_stmnt, &[name, scores, ratios, misses])
            .await?;
        let upsert_stmnt = txn
            .prepare_typed(
                upsert_query,
                &[
                    Type::BYTEA,
                    Type::CHAR_ARRAY,
                    Type::FLOAT4_ARRAY,
                    Type::FLOAT4_ARRAY,
                ],
            )
            .await?;
        txn.execute(upsert_stmnt, &[name, scores, ratios, misses])
            .await?;
        txn.commit().await?;
        let old_ratios = row.map(|row| Ratios {
            scores: row.get(0),
            ratios: row.get(1),
            misses: row.get(2),
        });
        Ok(old_ratios)
    }
}
