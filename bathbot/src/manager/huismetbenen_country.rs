use bathbot_util::CowUtils;
use rosu_v2::model::GameMode;

use super::redis::RedisManager;

#[derive(Copy, Clone)]
pub struct HuismetbenenCountryManager;

impl HuismetbenenCountryManager {
    pub fn new() -> Self {
        Self
    }

    #[allow(clippy::wrong_self_convention)]
    pub async fn is_supported(self, country_code: &str, mode: GameMode) -> bool {
        let country_code = country_code.cow_to_ascii_uppercase();

        match RedisManager::new().snipe_countries(mode).await {
            Ok(countries) => countries.contains(country_code.as_ref()),
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
}
