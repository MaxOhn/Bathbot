use std::sync::Arc;

use bathbot_util::CowUtils;
use rosu_v2::model::GameMode;

use super::redis::{RedisData, RedisManager};
use crate::core::Context;

#[derive(Clone)]
pub struct HuismetbenenCountryManager {
    ctx: Arc<Context>,
}

impl HuismetbenenCountryManager {
    pub fn new(ctx: Arc<Context>) -> Self {
        Self { ctx }
    }

    #[allow(clippy::wrong_self_convention)]
    pub async fn is_supported(self, country_code: &str, mode: GameMode) -> bool {
        let country_code = country_code.cow_to_ascii_uppercase();

        match RedisManager::new(self.ctx).snipe_countries(mode).await {
            Ok(RedisData::Original(countries)) => countries.contains(country_code.as_ref()),
            Ok(RedisData::Archive(countries)) => countries.contains(country_code.as_ref()),
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
