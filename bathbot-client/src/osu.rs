use bathbot_util::constants::OSU_BASE;
use bytes::Bytes;
use eyre::{Report, Result, WrapErr};
use http::response::Parts;
use hyper::{header::USER_AGENT, Request};

use crate::{client::Body, Client, ClientError, Site, MY_USER_AGENT};

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
