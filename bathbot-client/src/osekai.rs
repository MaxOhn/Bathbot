use bathbot_model::{
    OsekaiBadge, OsekaiBadgeOwner, OsekaiComment, OsekaiComments, OsekaiMap, OsekaiMaps,
    OsekaiMedal, OsekaiRanking, OsekaiRankingEntries,
};
use eyre::{Result, WrapErr};
use itoa::Buffer as IntBuffer;

use crate::{multipart::Multipart, site::Site, Client};

impl Client {
    /// Don't use this; use `RedisManager::badges` instead.
    pub async fn get_osekai_badges(&self) -> Result<Vec<OsekaiBadge>> {
        let url = "https://osekai.net/badges/api/getBadges.php";

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai badges: {body}")
        })
    }

    pub async fn get_osekai_badge_owners(&self, badge_id: u32) -> Result<Vec<OsekaiBadgeOwner>> {
        let url = format!("https://osekai.net/badges/api/getUsers.php?badge_id={badge_id}");
        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai badge owners: {body}")
        })
    }

    /// Don't use this; use `RedisManager::medals` instead.
    pub async fn get_osekai_medals(&self) -> Result<Vec<OsekaiMedal>> {
        let url = "https://osekai.net/medals/api/medals.php";

        let mut form = Multipart::new();
        form.push_text("strSearch", "");

        let bytes = self
            .make_multipart_post_request(url, Site::Osekai, form)
            .await?;

        serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize osekai medals: {body}")
        })
    }

    pub async fn get_osekai_beatmaps(&self, medal_name: &str) -> Result<Vec<OsekaiMap>> {
        let url = "https://osekai.net/medals/api/beatmaps.php";
        let mut form = Multipart::new();
        form.push_text("strSearch", medal_name);

        let bytes = self
            .make_multipart_post_request(url, Site::Osekai, form)
            .await?;

        let maps: OsekaiMaps = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai maps: {body}")
        })?;

        Ok(maps.0.unwrap_or_default())
    }

    pub async fn get_osekai_comments(&self, medal_id: u32) -> Result<Vec<OsekaiComment>> {
        let url = "https://osekai.net/global/api/comment_system.php";

        let mut buf = IntBuffer::new();
        let mut form = Multipart::new();
        form.push_int("strMedalID", medal_id, &mut buf)
            .push_text("bGetComments", "true");

        let bytes = self
            .make_multipart_post_request(url, Site::Osekai, form)
            .await?;

        let comments: OsekaiComments = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("failed to deserialize osekai comments: {body}")
        })?;

        Ok(comments.0.unwrap_or_default())
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

                format!("failed to deserialize Osekai {}: {body}", R::FORM)
            })
    }
}
