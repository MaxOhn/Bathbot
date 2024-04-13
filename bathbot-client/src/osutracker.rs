use bathbot_model::{OsuTrackerIdCount, OsuTrackerPpGroup, OsuTrackerStats};
use eyre::{Result, WrapErr};

use crate::{site::Site, Client};

impl Client {
    /// Don't use this; use `RedisManager::osutracker_stats` instead.
    pub async fn get_osutracker_stats(&self) -> Result<OsuTrackerStats> {
        let url = "https://osutracker.com/api/stats";
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker stats: {body}")
        })
    }

    /// Don't use this; use `RedisManager::osutracker_pp_group` instead.
    pub async fn get_osutracker_pp_group(&self, pp: u32) -> Result<OsuTrackerPpGroup> {
        let url = format!("https://osutracker.com/api/stats/ppBarrier?number={pp}");
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker pp groups: {body}")
        })
    }

    /// Don't use this; use `RedisManager::osutracker_counts` instead.
    pub async fn get_osutracker_counts(&self) -> Result<Vec<OsuTrackerIdCount>> {
        let url = "https://osutracker.com/api/stats/idCounts";
        let bytes = self.make_get_request(url, Site::OsuTracker).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osutracker id counts: {body}")
        })
    }
}
