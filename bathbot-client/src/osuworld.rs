use bathbot_model::{CountryRegions, OsuWorldUserIds};
use eyre::{Result, WrapErr};

use crate::{site::Site, Client};

impl Client {
    /// Don't use this; use `RedisManager::country_regions` instead.
    pub async fn get_country_regions(&self) -> Result<CountryRegions> {
        let url = "https://osuworld.octo.moe/locales/en/regions.json";
        let bytes = self.make_get_request(url, Site::OsuWorld).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize country regions: {body}")
        })
    }

    pub async fn get_region_user_ids(&self, region: &str) -> Result<Vec<i32>> {
        let url = format!("https://osuworld.octo.moe/api/bathbot/users/{region}");
        let bytes = self.make_get_request(url, Site::OsuWorld).await?;

        serde_json::from_slice(&bytes)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize region user ids: {body}")
            })
            .map(|OsuWorldUserIds(user_ids)| user_ids)
    }
}
