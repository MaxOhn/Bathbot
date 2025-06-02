use bathbot_model::{
    OsekaiBadge, OsekaiBadgeOwner, OsekaiBadgeOwners, OsekaiComment, OsekaiInex, OsekaiMap,
    OsekaiMedal, OsekaiRanking, OsekaiRankingEntries,
};
use eyre::{Result, WrapErr};

use crate::{Client, multipart::Multipart, site::Site};

impl Client {
    /// Don't use this; use `RedisManager::badges` instead.
    pub async fn get_osekai_badges(&self) -> Result<Vec<OsekaiBadge>> {
        let url = "https://osekai.net/badges/api/getBadges.php";

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize: {body}")
        })
    }

    pub async fn get_osekai_badge_owners(&self, badge_id: u32) -> Result<Vec<OsekaiBadgeOwner>> {
        let url = format!("https://osekai.net/badges/api/getUsers.php?badge_id={badge_id}");
        let bytes = self.make_get_request(url, Site::Osekai).await?;

        let OsekaiBadgeOwners(owners) = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize: {body}")
        })?;

        Ok(owners)
    }

    /// Don't use this; use `RedisManager::medals` instead.
    ///
    /// Medals will be sorted by medal id.
    pub async fn get_osekai_medals(&self) -> Result<Vec<OsekaiMedal>> {
        let url = "https://inex.osekai.net/api/medals/get_all";

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice::<OsekaiInex<Vec<OsekaiMedal>>>(&bytes)
            .map(|inex| {
                let mut medals = inex.content;
                medals.sort_unstable_by_key(|medal| medal.medal_id);

                medals
            })
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }

    pub async fn get_osekai_beatmaps(&self, medal_id: u32) -> Result<Vec<OsekaiMap>> {
        let url = format!("https://inex.osekai.net/api/medals/{medal_id}/beatmaps");

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice::<OsekaiInex<Vec<OsekaiMap>>>(&bytes)
            .map(|inex| inex.content)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }

    pub async fn get_osekai_comments(&self, medal_id: u32) -> Result<Vec<OsekaiComment>> {
        let url = format!("https://inex.osekai.net/api/comments/Medals_Data/{medal_id}/get");

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice::<OsekaiInex<Vec<OsekaiComment>>>(&bytes)
            .map(|inex| inex.content)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }

    /// Don't use this; use `RedisManager::osekai_ranking` instead.
    pub async fn get_osekai_ranking<R: OsekaiRanking>(&self) -> Result<Vec<R::Entry>> {
        let url = "https://osekai.net/rankings/api/api.php";

        let mut form = Multipart::new();
        form.push_text("App", R::FORM);

        let bytes = self
            .make_multipart_post_request(url, Site::Osekai, form)
            .await?;

        serde_json::from_slice::<OsekaiRankingEntries<R>>(&bytes)
            .map(Vec::from)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }
}
