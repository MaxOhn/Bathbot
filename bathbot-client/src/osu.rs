use std::fmt::{Formatter, Result as FmtResult};

use base64::{engine::general_purpose::STANDARD, Engine};
use bathbot_util::constants::OSU_BASE;
use bytes::Bytes;
use eyre::{Report, Result, WrapErr};
use http::{header::USER_AGENT, Method, Request, Response};
use hyper::Body;
use serde::{
    de::{Error as DeError, Visitor},
    Deserialize, Deserializer,
};

use crate::{Client, ClientError, Site, MY_USER_AGENT};

impl Client {
    pub async fn check_skin_url(&self, url: &str) -> Result<Response<Body>, ClientError> {
        trace!("HEAD request of url {url}");

        let req = Request::builder()
            .uri(url)
            .method(Method::HEAD)
            .header(USER_AGENT, MY_USER_AGENT)
            .body(Body::empty())
            .wrap_err("failed to build HEAD request")?;

        let response = self
            .client
            .request(req)
            .await
            .wrap_err("failed to receive HEAD response")?;

        let status = response.status();

        if (200..=299).contains(&status.as_u16()) {
            Ok(response)
        } else {
            Err(eyre!("failed with status code {status} when requesting url {url}").into())
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

    pub async fn get_raw_osu_replay(&self, key: &str, score_id: u64) -> Result<Option<Box<[u8]>>> {
        #[derive(Deserialize)]
        struct RawReplayBody {
            #[serde(default, rename = "content", deserialize_with = "decode_base64")]
            decoded: Option<Box<[u8]>>,
        }

        fn decode_base64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Box<[u8]>>, D::Error> {
            struct RawReplayVisitor;

            impl<'de> Visitor<'de> for RawReplayVisitor {
                type Value = Box<[u8]>;

                fn expecting(&self, f: &mut Formatter<'_>) -> FmtResult {
                    f.write_str("a base64 encoded string")
                }

                fn visit_str<E: DeError>(self, v: &str) -> Result<Self::Value, E> {
                    STANDARD
                        .decode(v)
                        .map(Vec::into_boxed_slice)
                        .map_err(|e| DeError::custom(format!("Failed to decode base64: {e}")))
                }
            }

            d.deserialize_str(RawReplayVisitor).map(Some)
        }

        let url = format!("https://osu.ppy.sh/api/get_replay?k={key}&s={score_id}");

        let bytes = self.make_get_request(url, Site::OsuReplay).await?;

        let RawReplayBody { decoded } = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize replay body: {body}")
        })?;

        Ok(decoded)
    }
}
