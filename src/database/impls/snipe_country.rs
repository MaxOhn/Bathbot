use eyre::Result;
use flurry::HashMap as FlurryMap;
use futures::stream::StreamExt;

use crate::{util::CountryCode, Database};

impl Database {
    #[cold]
    pub async fn get_snipe_countries(&self) -> Result<FlurryMap<CountryCode, String>> {
        let mut stream = sqlx::query!("SELECT * FROM snipe_countries").fetch(&self.pool);
        let countries = FlurryMap::with_capacity(128);

        {
            let guard = countries.guard();

            while let Some(entry) = stream.next().await.transpose()? {
                let country = entry.name;
                let code = entry.code;

                countries.insert(code.into(), country, &guard);
            }
        }

        Ok(countries)
    }

    pub async fn insert_snipe_country(&self, country: &str, code: &str) -> Result<()> {
        sqlx::query!("INSERT INTO snipe_countries VALUES ($1,$2)", country, code)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
