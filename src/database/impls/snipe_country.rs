use crate::{core::Country, BotResult, Database};

use dashmap::DashMap;
use futures::stream::StreamExt;

impl Database {
    #[cold]
    pub async fn get_snipe_countries(&self) -> BotResult<DashMap<String, Country>> {
        let mut stream = sqlx::query!("SELECT * FROM snipe_countries").fetch(&self.pool);
        let mut countries = DashMap::with_capacity(128);

        while let Some(entry) = stream.next().await.transpose()? {
            let country = entry.name;
            let code = entry.code;

            countries.insert(country, code.into());
        }

        Ok(countries)
    }

    pub async fn insert_snipe_country(&self, country: &str, code: &str) -> BotResult<()> {
        sqlx::query!("INSERT INTO snipe_countries VALUES ($1, $2)", country, code)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
