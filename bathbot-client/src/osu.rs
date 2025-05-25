use bathbot_model::{ScrapedMedal, ScrapedUser};
use bathbot_util::{constants::OSU_BASE, html::decode_html_entities};
use bytes::Bytes;
use eyre::{ContextCompat, Report, Result, WrapErr};
use http::response::Parts;
use hyper::{Request, header::USER_AGENT};

use crate::{Client, ClientError, MY_USER_AGENT, Site, client::Body};

impl Client {
    pub async fn check_skin_url(&self, url: &str) -> Result<Parts, ClientError> {
        trace!("HEAD request of url {url}");

        let req = Request::head(url)
            .header(USER_AGENT, MY_USER_AGENT)
            .body(Body::default())
            .wrap_err("Failed to build HEAD request")?;

        let response = self
            .client
            .request(req)
            .await
            .wrap_err("failed to receive HEAD response")?;

        let status = response.status();

        if (200..=299).contains(&status.as_u16()) {
            let (parts, _) = response.into_parts();

            Ok(parts)
        } else {
            Err(eyre!("Failed with status code {status} when requesting url {url}").into())
        }
    }

    pub async fn get_avatar(&self, url: &str) -> Result<Bytes> {
        self.make_get_request(url, Site::OsuAvatar)
            .await
            .map_err(Report::new)
    }

    pub async fn get_badge(&self, url: &str) -> Result<Bytes> {
        self.make_get_request(url, Site::OsuBadge)
            .await
            .map_err(Report::new)
    }

    pub async fn get_flag(&self, url: &str) -> Result<Bytes> {
        self.make_get_request(url, Site::Flags)
            .await
            .map_err(Report::new)
    }

    pub async fn get_medal_icon(&self, url: &str) -> Result<Bytes> {
        self.make_get_request(url, Site::OsuMedalIcon)
            .await
            .map_err(Report::new)
    }

    /// Don't use this; use `RedisManager::scraped_medals` instead.
    pub async fn get_medals(&self) -> Result<Box<[ScrapedMedal]>> {
        const KEY: &str = "data-initial-data=";

        let bytes = self.peppy_profile().await?;
        let data = std::str::from_utf8(&bytes)?;
        let start = data.find(KEY).wrap_err("missing key")? + KEY.len() + 1;
        let end = memchr::memchr(b'"', &bytes[start..]).wrap_err("missing end quote")? + start;

        let data_initial_data = &data[start..end];
        let decoded = decode_html_entities(data_initial_data);

        let ScrapedUser { medals } = serde_json::from_str(&decoded)
            .wrap_err_with(|| format!("Failed to deserialize: {decoded}"))?;

        Ok(medals)
    }

    async fn peppy_profile(&self) -> Result<Bytes> {
        let url = "https://osu.ppy.sh/users/2";

        self.make_get_request(url, Site::OsuProfile)
            .await
            .map_err(Report::new)
    }

    /// Make sure you provide a valid url to a mapset cover
    pub async fn get_mapset_cover(&self, cover: &str) -> Result<Bytes> {
        self.make_get_request(&cover, Site::OsuMapsetCover)
            .await
            .map_err(Report::new)
    }

    pub async fn get_map_file(&self, map_id: u32) -> Result<Bytes, ClientError> {
        let url = format!("{OSU_BASE}osu/{map_id}");

        self.make_get_request(&url, Site::OsuMapFile).await
    }
}
