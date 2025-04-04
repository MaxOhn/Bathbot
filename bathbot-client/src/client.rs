use std::time::Instant;

use bytes::Bytes;
use eyre::{Result, WrapErr};
use http_body_util::{BodyExt, Collected, Full};
use hyper::{
    Method, Request, Response,
    body::Incoming,
    header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, USER_AGENT},
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Builder, Client as HyperClient, Error as HyperError, connect::HttpConnector},
    rt::TokioExecutor,
};
use leaky_bucket_lite::LeakyBucket;
use tokio::time::Duration;

use crate::{ClientError, MY_USER_AGENT, Site, metrics::ClientMetrics, multipart::Multipart};

pub(crate) type InnerClient = HyperClient<HttpsConnector<HttpConnector>, Body>;
pub(crate) type Body = Full<Bytes>;

pub struct Client {
    pub(crate) client: InnerClient,
    #[cfg(feature = "twitch")]
    twitch: bathbot_model::TwitchData,
    github_auth: Box<str>,
    ratelimiters: [LeakyBucket; 18],
}

impl Client {
    pub async fn new(
        #[cfg(feature = "twitch")] (twitch_client_id, twitch_token): (&str, &str),
        github_token: &str,
    ) -> Result<Self> {
        ClientMetrics::init();

        let crypto_provider = rustls::crypto::ring::default_provider();

        let https = HttpsConnectorBuilder::new()
            .with_provider_and_webpki_roots(crypto_provider)
            .wrap_err("Failed to configure https connector")?
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build();

        let client = Builder::new(TokioExecutor::new()).build(https);

        #[cfg(feature = "twitch")]
        let twitch = Self::get_twitch_token(&client, twitch_client_id, twitch_token)
            .await
            .wrap_err("failed to get twitch token")?;

        let ratelimiter = |per_second| {
            LeakyBucket::builder()
                .max(per_second)
                .tokens(per_second)
                .refill_interval(Duration::from_millis(1000 / per_second as u64))
                .refill_amount(1)
                .build()
        };

        let github_auth = format!("Bearer {github_token}").into_boxed_str();

        let ratelimiters = [
            ratelimiter(2),  // DiscordAttachment
            ratelimiter(10), // Flags
            ratelimiter(5),  // Github
            ratelimiter(2),  // Huismetbenen
            ratelimiter(5),  // KittenRoleplay
            ratelimiter(5),  // MissAnalyzer
            ratelimiter(2),  // Osekai
            ratelimiter(10), // OsuAvatar
            ratelimiter(10), // OsuBadge
            ratelimiter(2),  // OsuMapFile
            ratelimiter(10), // OsuMapsetCover
            LeakyBucket::builder() // OsuReplay, allows 6 per minute
                .max(10)
                .tokens(10)
                .refill_interval(Duration::from_secs(7))
                .refill_amount(1)
                .build(),
            ratelimiter(2), // OsuStats
            ratelimiter(2), // OsuTrack
            ratelimiter(2), // OsuWorld
            ratelimiter(1), // Respektive
            ratelimiter(2), // Relaxation Vault
            ratelimiter(5), // Twitch
        ];

        Ok(Self {
            client,
            ratelimiters,
            #[cfg(feature = "twitch")]
            twitch,
            github_auth,
        })
    }

    pub(crate) async fn ratelimit(&self, site: Site) {
        self.ratelimiters[site as usize].acquire_one().await
    }

    pub(crate) async fn make_get_request(
        &self,
        url: impl AsRef<str>,
        site: Site,
    ) -> Result<Bytes, ClientError> {
        let url = url.as_ref();
        trace!("GET request to url {url}");

        let req = Request::builder()
            .uri(url)
            .method(Method::GET)
            .header(USER_AGENT, MY_USER_AGENT);

        let req = match site {
            #[cfg(not(feature = "twitch"))]
            Site::Twitch => {
                return Err(ClientError::Report(eyre::Report::msg(
                    "twitch request without twitch feature",
                )));
            }
            #[cfg(feature = "twitch")]
            Site::Twitch => req
                .header("Client-ID", self.twitch.client_id.clone())
                .header(
                    http::header::AUTHORIZATION,
                    format!("Bearer {}", self.twitch.oauth_token),
                ),
            _ => req,
        };

        let req = req
            .body(Body::default())
            .wrap_err("failed to build GET request")?;

        let (response, start) = self
            .send_request(req, site)
            .await
            .wrap_err("failed to receive GET response")?;

        let status = response.status();
        let bytes_res = Self::error_for_status(response, url).await;

        let latency = start.elapsed();
        ClientMetrics::observe(site, status, latency);

        bytes_res
    }

    pub(crate) async fn make_multipart_post_request(
        &self,
        url: impl AsRef<str>,
        site: Site,
        form: Multipart,
    ) -> Result<Bytes, ClientError> {
        let url = url.as_ref();
        trace!("POST multipart request to url {url}");

        let content_type = form.content_type();
        let content = form.build();

        let req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header(USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, content_type)
            .header(CONTENT_LENGTH, content.len())
            .body(Body::from(content))
            .wrap_err("Failed to build POST request")?;

        self.ratelimit(site).await;

        let (response, start) = self
            .send_request(req, site)
            .await
            .wrap_err("Failed to receive POST multipart response")?;

        let status = response.status();
        let bytes_res = Self::error_for_status(response, url).await;

        let latency = start.elapsed();
        ClientMetrics::observe(site, status, latency);

        bytes_res
    }

    pub(crate) async fn make_json_post_request(
        &self,
        url: impl AsRef<str>,
        site: Site,
        json: Vec<u8>,
    ) -> Result<Bytes, ClientError> {
        let url = url.as_ref();
        trace!("POST json request to url {url}");

        let mut req = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header(USER_AGENT, MY_USER_AGENT)
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, json.len());

        if site == Site::Github {
            req = req.header(AUTHORIZATION, self.github_auth.as_ref());
        }

        let req = req
            .body(Body::from(json))
            .wrap_err("Failed to build POST json request")?;

        self.ratelimit(site).await;

        let (response, start) = self
            .send_request(req, site)
            .await
            .wrap_err("Failed to receive POST response")?;

        let status = response.status();
        let bytes_res = Self::error_for_status(response, url).await;

        let latency = start.elapsed();
        ClientMetrics::observe(site, status, latency);

        bytes_res
    }

    pub(crate) async fn error_for_status(
        response: Response<Incoming>,
        url: &str,
    ) -> Result<Bytes, ClientError> {
        let status = response.status();

        match status.as_u16() {
            200..=299 => response
                .into_body()
                .collect()
                .await
                .map(Collected::to_bytes)
                .wrap_err("Failed to collect response bytes")
                .map_err(ClientError::Report),
            400 => Err(ClientError::BadRequest),
            404 => Err(ClientError::NotFound),
            429 => Err(ClientError::Ratelimited),
            _ => Err(eyre!("Failed with status code {status} when requesting url {url}").into()),
        }
    }

    async fn send_request(
        &self,
        req: Request<Body>,
        site: Site,
    ) -> Result<(Response<Incoming>, Instant), HyperError> {
        self.ratelimit(site).await;

        let start = Instant::now();
        let response_fut = self.client.request(req);

        match response_fut.await {
            Ok(res) => Ok((res, start)),
            Err(err) => {
                ClientMetrics::internal_error(site);

                Err(err)
            }
        }
    }
}
