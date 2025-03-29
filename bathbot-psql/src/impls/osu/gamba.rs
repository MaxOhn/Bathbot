use std::collections::HashMap;

use eyre::{Result, WrapErr};

use crate::Database;

impl Database {
    pub async fn select_bathcoin_amount(&self, user_id: u32) -> Result<Option<u64>> {
        sqlx::query!(
            "SELECT amount FROM bathcoins WHERE osu_id = $1",
            user_id as i32
        )
        .fetch_optional(self)
        .await
        .map(|opt| opt.map(|row| row.amount as u64))
        .wrap_err("Failed to fetch optional")
    }

    pub async fn increase_single_bathcoins(&self, user_id: u32, amount: u64) -> Result<u64> {
        sqlx::query!(
            r#"
INSERT INTO bathcoins
VALUES ($1, $2)
ON CONFLICT (osu_id)
DO UPDATE SET amount = bathcoins.amount + $2
RETURNING amount"#,
            user_id as i32,
            amount as i64
        )
        .fetch_one(self)
        .await
        .map(|row| row.amount as u64)
        .wrap_err("Failed to fetch data")
    }

    pub async fn increase_multi_bathcoins<S>(&self, user_ids: &HashMap<u32, u64, S>) -> Result<()> {
        let (user_ids, amounts): (Vec<_>, Vec<_>) = user_ids
            .iter()
            .filter(|&(_, &amount)| amount > 0)
            .map(|(&id, &amount)| (id as i32, amount as i64))
            .collect();

        sqlx::query!(
            r#"
INSERT INTO bathcoins
VALUES (UNNEST($1::INT4[]), UNNEST($2::INT8[]))
ON CONFLICT (osu_id)
DO UPDATE SET amount = bathcoins.amount + excluded.amount"#,
            &user_ids,
            &amounts,
        )
        .execute(self)
        .await
        .wrap_err("Failed to execute query")?;

        Ok(())
    }

    /// Returns the amount *BEFORE* decreasing
    pub async fn decrease_bathcoin_amount(&self, user_id: u32, amount: u64) -> Result<u64> {
        let res = sqlx::query!(
            r#"
UPDATE bathcoins AS new
SET amount =
    CASE WHEN new.amount >= $2
    THEN new.amount - $2
    ELSE new.amount END
FROM
    (SELECT osu_id, amount
    FROM bathcoins
    WHERE osu_id = $1
    FOR UPDATE) AS old
WHERE new.osu_id = old.osu_id
RETURNING old.amount"#,
            user_id as i32,
            amount as i64,
        )
        .fetch_optional(self)
        .await
        .wrap_err("Failed to fetch optional")?;

        match res {
            Some(row) => Ok(row.amount as u64),
            None => Ok(0),
        }
    }
}
