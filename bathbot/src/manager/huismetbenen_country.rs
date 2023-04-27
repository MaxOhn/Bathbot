use bathbot_psql::Database;
use bathbot_util::CowUtils;
use eyre::{Result, WrapErr};

#[derive(Copy, Clone)]
pub struct HuismetbenenCountryManager<'d> {
    psql: &'d Database,
}

impl<'d> HuismetbenenCountryManager<'d> {
    pub fn new(psql: &'d Database) -> Self {
        Self { psql }
    }

    pub async fn is_supported(self, country_code: &str) -> bool {
        let country_code = country_code.cow_to_ascii_uppercase();

        let is_supported_fut = self
            .psql
            .select_contains_huismetbenen_country(country_code.as_ref());

        match is_supported_fut.await {
            Ok(is_supported) => is_supported,
            Err(err) => {
                warn!(
                    country_code = country_code.as_ref(),
                    ?err,
                    "Failed to check if country code contained"
                );

                false
            }
        }
    }

    pub async fn get_country(self, country_code: &str) -> Option<String> {
        match self.psql.select_huismetbenen_country(country_code).await {
            Ok(country_name) => country_name,
            Err(err) => {
                warn!(country_code, ?err, "Failed to get huismetbenen country");

                None
            }
        }
    }

    pub async fn add_country(self, country_code: &str, country_name: &str) -> Result<()> {
        self.psql
            .upsert_huismetbenen_country(country_code, country_name)
            .await
            .wrap_err("failed to upsert huismetbenen country")
    }
}
