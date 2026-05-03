use bathbot_model::{
    MedalCount, OsekaiBadge, OsekaiBadges, OsekaiComment, OsekaiInex, OsekaiMap, OsekaiMedal,
    OsekaiRanking, OsekaiRankingEntries, OsekaiRankingEntry, OsekaiRarityEntry, OsekaiUserEntry,
    Rarity,
};

use eyre::{Result, WrapErr};
use serde::Serialize;

use crate::{Client, site::Site};

impl Client {
    /// Don't use this; use `RedisManager::badges` instead.
    ///
    /// When `compress` is `true`, the API returns a compressed object format:
    /// `{"content": {"_t":true,"k":[...],"d":[...]}}`
    pub async fn get_osekai_badges(&self) -> Result<Vec<OsekaiBadge>> {
        let url = "https://inex.osekai.net/api/badges/get_all?compress=true";

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice::<OsekaiInex<OsekaiBadges>>(&bytes)
            .map(|inex| inex.content.0)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
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
    pub async fn get_osekai_ranking(
        &self,
        ranking_kind: &str,
        ranking_options_kind: Option<&str>,
    ) -> Result<Vec<OsekaiRankingEntry>> {
        let url = "https://inex.osekai.net/api/rankings/get";

        let json = serde_json::to_vec(&OsekaiRankingBody::new(ranking_kind, ranking_options_kind))
            .unwrap();

        let bytes = self.make_json_post_request(url, Site::Osekai, json).await?;

        serde_json::from_slice::<OsekaiInex<OsekaiRankingEntries<OsekaiRankingEntry>>>(&bytes)
            .map(|inex| inex.content.data.0)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }

    /// Don't use this; use `RedisManager::osekai_medal_count` instead.
    pub async fn get_osekai_medal_count(&self) -> Result<Vec<OsekaiUserEntry>> {
        let url = "https://inex.osekai.net/api/rankings/get";

        let json = serde_json::to_vec(&OsekaiRankingBody::new(
            MedalCount::KIND,
            MedalCount::OPTIONS_KIND,
        ))
        .unwrap();

        let bytes = self.make_json_post_request(url, Site::Osekai, json).await?;

        serde_json::from_slice::<OsekaiInex<OsekaiRankingEntries<OsekaiUserEntry>>>(&bytes)
            .map(|inex| inex.content.data.0)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }

    /// Don't use this; use `RedisManager::osekai_rarity` instead.
    pub async fn get_osekai_rarity(&self) -> Result<Vec<OsekaiRarityEntry>> {
        let url = "https://inex.osekai.net/api/rankings/get";

        let json = serde_json::to_vec(&OsekaiRankingBody::new(Rarity::KIND, Rarity::OPTIONS_KIND))
            .unwrap();

        let bytes = self.make_json_post_request(url, Site::Osekai, json).await?;

        serde_json::from_slice::<OsekaiInex<OsekaiRankingEntries<OsekaiRarityEntry>>>(&bytes)
            .map(|inex| inex.content.data.0)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }
}

#[derive(Serialize)]
struct OsekaiRankingBody<'a> {
    compress: bool,
    offset: u32,
    options: OsekaiRankingBodyOptions<'a>,
    #[serde(rename = "type")]
    kind: &'a str,
}

#[derive(Serialize)]
struct OsekaiRankingBodyOptions<'a> {
    #[serde(rename = "queryColumn")]
    query_column: &'static str,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<&'a str>,
}

impl<'a> OsekaiRankingBody<'a> {
    fn new(kind: &'a str, options_kind: Option<&'a str>) -> Self {
        Self {
            compress: false,
            offset: 0,
            options: OsekaiRankingBodyOptions {
                query_column: "Username",
                kind: options_kind,
            },
            kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_osekai_badges_integration() {
        let client = Client::new("").await.unwrap();

        let badges = client.get_osekai_badges().await.unwrap();

        assert!(
            !badges.is_empty(),
            "Expected at least one badge from the API"
        );

        let first = &badges[0];
        assert!(first.badge_id != 0, "Badge id should be non-zero");
        assert!(!first.name.is_empty(), "Badge name should not be empty");
    }
}
