use bathbot_util::CowUtils;

use super::redis::RedisData;
use crate::core::Context;

#[derive(Copy, Clone)]
pub struct HuismetbenenCountryManager<'c> {
    ctx: &'c Context,
}

impl<'c> HuismetbenenCountryManager<'c> {
    pub fn new(ctx: &'c Context) -> Self {
        Self { ctx }
    }

    pub async fn is_supported(self, country_code: &str) -> bool {
        let country_code = country_code.cow_to_ascii_uppercase();

        match self.ctx.redis().snipe_countries().await {
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
