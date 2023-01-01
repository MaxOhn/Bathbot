use eyre::{Result, WrapErr};

use crate::database::Database;

impl Database {
    pub async fn select_contains_huismetbenen_country(&self, country_code: &str) -> Result<bool> {
        let query = sqlx::query!(
            r#"
SELECT 
  EXISTS (
    SELECT 
    FROM 
      huismetbenen_countries 
    WHERE 
      country_code = $1
  )"#,
            country_code
        );

        let row = query
            .fetch_one(self)
            .await
            .wrap_err("failed to fetch one")?;

        Ok(row.exists.unwrap_or(false))
    }

    pub async fn select_huismetbenen_country(&self, country_code: &str) -> Result<Option<String>> {
        let query = sqlx::query!(
            r#"
SELECT 
  country_name 
FROM 
  huismetbenen_countries 
WHERE 
  country_code = $1"#,
            country_code
        );

        let row_opt = query
            .fetch_optional(self)
            .await
            .wrap_err("failed to fetch optional")?;

        Ok(row_opt.map(|row| row.country_name))
    }

    pub async fn upsert_huismetbenen_country(
        &self,
        country_code: &str,
        country_name: &str,
    ) -> Result<()> {
        let query = sqlx::query!(
            r#"
INSERT INTO huismetbenen_countries (country_code, country_name) 
VAlUES 
  ($1, $2) ON CONFLICT (country_code) DO 
UPDATE 
SET 
  country_name = $2"#,
            country_code,
            country_name
        );

        query
            .execute(self)
            .await
            .wrap_err("failed to execute query")?;

        Ok(())
    }
}
