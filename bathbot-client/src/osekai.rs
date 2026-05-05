use std::num::NonZeroU8;

use bathbot_model::{
    CompactWrap, MedalCount, OsekaiBadge, OsekaiBadges, OsekaiComment, OsekaiInex, OsekaiMap,
    OsekaiMedal, OsekaiRanking, OsekaiRankingEntries, OsekaiRankingEntry, OsekaiRarityEntry,
    OsekaiUserEntry, Rarity,
};
use eyre::{Result, WrapErr};
use serde::Serialize;

use crate::{Client, site::Site};

const RANKING_PER_PAGE: usize = 50;

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
        let url = "https://inex.osekai.net/api/medals/get_all?compress=true";

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice::<OsekaiInex<CompactWrap<OsekaiMedal>>>(&bytes)
            .map(|inex| {
                let mut medals = inex.content.0;
                medals.sort_unstable_by_key(|medal| medal.medal_id);

                medals
            })
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }

    pub async fn get_osekai_beatmaps(&self, medal_id: u32) -> Result<Vec<OsekaiMap>> {
        let url = format!("https://inex.osekai.net/api/medals/{medal_id}/beatmaps?compress=true");

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice::<OsekaiInex<CompactWrap<OsekaiMap>>>(&bytes)
            .map(|inex| inex.content.0)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }

    pub async fn get_osekai_comments(&self, medal_id: u32) -> Result<Vec<OsekaiComment>> {
        let url = format!(
            "https://inex.osekai.net/api/comments/Medals_Data/{medal_id}/get?compress=true"
        );

        let bytes = self.make_get_request(url, Site::Osekai).await?;

        serde_json::from_slice::<OsekaiInex<CompactWrap<OsekaiComment>>>(&bytes)
            .map(|inex| inex.content.0)
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
        country: Option<&str>,
        page: NonZeroU8,
    ) -> Result<OsekaiRankingEntries<OsekaiRankingEntry>> {
        let url = "https://inex.osekai.net/api/rankings/get";

        let mut body = OsekaiRankingBody::new(ranking_kind);

        if let Some(options_kind) = ranking_options_kind {
            body.with_option_kind(options_kind);
        }

        if let Some(country) = country {
            body.with_country(country);
        }

        let offset = usize::from(page.get() - 1) * RANKING_PER_PAGE;
        let json = serde_json::to_vec(body.with_offset(offset)).unwrap();
        let bytes = self.make_json_post_request(url, Site::Osekai, json).await?;

        serde_json::from_slice::<OsekaiInex<OsekaiRankingEntries<OsekaiRankingEntry>>>(&bytes)
            .map(|inex| inex.content)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }

    /// Don't use this; use `RedisManager::osekai_medal_count` instead.
    pub async fn get_osekai_medal_count(
        &self,
        country: Option<&str>,
        page: NonZeroU8,
    ) -> Result<OsekaiRankingEntries<OsekaiUserEntry>> {
        let url = "https://inex.osekai.net/api/rankings/get";

        let mut body = OsekaiRankingBody::new(MedalCount::KIND);

        if let Some(country) = country {
            body.with_country(country);
        }

        let offset = usize::from(page.get() - 1) * RANKING_PER_PAGE;
        let json = serde_json::to_vec(body.with_offset(offset)).unwrap();
        let bytes = self.make_json_post_request(url, Site::Osekai, json).await?;

        serde_json::from_slice::<OsekaiInex<OsekaiRankingEntries<OsekaiUserEntry>>>(&bytes)
            .map(|inex| inex.content)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize: {body}")
            })
    }

    /// Don't use this; use `RedisManager::osekai_rarity` instead.
    ///
    /// Requests *all* pages and returns the full list. This is acceptable
    /// because there are only ~400 medals which means 8-9 requests if there are
    /// 50 medals per page.
    pub async fn get_osekai_rarity(&self) -> Result<Vec<OsekaiRarityEntry>> {
        let url = "https://inex.osekai.net/api/rankings/get";

        let mut entries = Vec::with_capacity(512);

        let mut offset = 0;
        let mut body = OsekaiRankingBody::new(Rarity::KIND);

        let mut json_buf = Vec::with_capacity(128);

        loop {
            body.with_offset(offset);
            serde_json::to_writer(&mut json_buf, &body).unwrap();

            let bytes = self
                .make_json_post_request(url, Site::Osekai, json_buf.clone())
                .await?;

            let inex: OsekaiInex<OsekaiRankingEntries<OsekaiRarityEntry>> =
                serde_json::from_slice(&bytes).wrap_err_with(|| {
                    let body = String::from_utf8_lossy(&bytes);

                    format!("Failed to deserialize: {body}")
                })?;

            if inex.content.data.is_empty() {
                break;
            }

            offset += inex.content.data.len();

            entries.extend(inex.content.data);

            if entries.len() >= inex.content.max as usize {
                break;
            }

            json_buf.clear();
        }

        Ok(entries)
    }
}

#[derive(Serialize)]
struct OsekaiRankingBody<'a> {
    compress: bool,
    offset: usize,
    options: OsekaiRankingBodyOptions<'a>,
    #[serde(rename = "type")]
    kind: &'a str,
}

#[derive(Serialize)]
struct OsekaiRankingBodyOptions<'a> {
    #[serde(rename = "queryColumn", skip_serializing_if = "Option::is_none")]
    query_column: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<&'a str>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<&'a str>,
}

impl<'a> OsekaiRankingBody<'a> {
    const fn new(kind: &'a str) -> Self {
        Self {
            compress: true,
            offset: 0,
            options: OsekaiRankingBodyOptions {
                query_column: None,
                query: None,
                kind: None,
            },
            kind,
        }
    }

    const fn with_offset(&mut self, offset: usize) -> &mut Self {
        self.offset = offset;

        self
    }

    const fn with_country(&mut self, country: &'a str) -> &mut Self {
        self.options.query = Some(country);
        self.options.query_column = Some("Country");

        self
    }

    const fn with_option_kind(&mut self, kind: &'a str) -> &mut Self {
        self.options.kind = Some(kind);

        self
    }
}
