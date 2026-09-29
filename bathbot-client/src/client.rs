#[cfg(feature = "twitch")]
use std::sync::Mutex;
use std::time::Instant;

use bytes::Bytes;
use eyre::{Result, WrapErr};
use http_body_util::{BodyExt, Collected, Full};
#[cfg(feature = "twitch")]
use hyper::StatusCode;
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

use crate::{
    ClientError, MY_USER_AGENT, Ratelimiters, Site, metrics::ClientMetrics, multipart::Multipart,
};

pub(crate) type InnerClient = HyperClient<HttpsConnector<HttpConnector>, Body>;
pub(crate) type Body = Full<Bytes>;

pub struct Client {
    pub(crate) client: InnerClient,
    #[cfg(feature = "twitch")]
    twitch: Mutex<bathbot_model::TwitchData>,
    #[cfg(feature = "twitch")]
    twitch_client_id: Box<str>,
    #[cfg(feature = "twitch")]
    twitch_secret: Box<str>,
    #[cfg(feature = "twitch")]
    twitch_token_url: Box<str>,
    github_auth: Box<str>,
    ratelimiters: Ratelimiters,
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
        let twitch = Self::get_twitch_token(
            &client,
            bathbot_util::constants::TWITCH_OAUTH,
            twitch_client_id,
            twitch_token,
        )
        .await
        .wrap_err("failed to get twitch token")?;

        Ok(Self {
            client,
            ratelimiters: Ratelimiters::new(),
            #[cfg(feature = "twitch")]
            twitch: Mutex::new(twitch),
            #[cfg(feature = "twitch")]
            twitch_client_id: twitch_client_id.to_owned().into_boxed_str(),
            #[cfg(feature = "twitch")]
            twitch_secret: twitch_token.to_owned().into_boxed_str(),
            #[cfg(feature = "twitch")]
            twitch_token_url: bathbot_util::constants::TWITCH_OAUTH
                .to_owned()
                .into_boxed_str(),
            github_auth: format!("Bearer {github_token}").into_boxed_str(),
        })
    }

    pub(crate) async fn ratelimit(&self, site: Site) {
        self.ratelimiters.get(site).acquire_one().await
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
            Site::Twitch => return self.twitch_get_request(url).await,
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

    /// Like `make_get_request`, but re-authenticates on a 401 response and
    /// retries once. Twitch app-only tokens have no refresh token; the
    /// documented recovery for an expired/invalidated token is simply
    /// issuing the same client-credentials request again.
    #[cfg(feature = "twitch")]
    async fn twitch_get_request(&self, url: &str) -> Result<Bytes, ClientError> {
        let mut refreshed = false;

        loop {
            let auth = {
                let twitch = self.twitch.lock().unwrap();

                format!("Bearer {}", twitch.oauth_token)
            };

            let req = Request::builder()
                .uri(url)
                .method(Method::GET)
                .header(USER_AGENT, MY_USER_AGENT)
                .header("Client-ID", self.twitch_client_id.as_ref())
                .header(AUTHORIZATION, auth)
                .body(Body::default())
                .wrap_err("Failed to build GET request")?;

            let (response, start) = self
                .send_request(req, Site::Twitch)
                .await
                .wrap_err("Failed to receive GET response")?;

            let status = response.status();

            if !refreshed && status == StatusCode::UNAUTHORIZED {
                refreshed = true;

                let twitch = Self::get_twitch_token(
                    &self.client,
                    self.twitch_token_url.as_ref(),
                    &self.twitch_client_id,
                    &self.twitch_secret,
                )
                .await
                .wrap_err("Failed to refresh twitch token")?;

                *self.twitch.lock().unwrap() = twitch;

                continue;
            }

            let latency = start.elapsed();
            ClientMetrics::observe(Site::Twitch, status, latency);

            return Self::error_for_status(response, url).await;
        }
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

#[cfg(test)]
#[cfg(feature = "twitch")]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use axum::{
        Router,
        http::StatusCode,
        routing::{get, post},
    };
    use hyper_rustls::HttpsConnectorBuilder;
    use hyper_util::{client::legacy::Builder, rt::TokioExecutor};

    use super::*;

    /// Mock Twitch API:
    /// - `POST /token`             -> 200 `{"access_token":"fresh"}`
    /// - `GET /data` (Bearer stale) -> 401
    /// - `GET /data` (Bearer fresh) -> 200 `ok`
    async fn start_mock_twitch(token_requested: Arc<AtomicBool>) -> u16 {
        async fn data(req: axum::extract::Request) -> (StatusCode, &'static str) {
            let auth = req
                .headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default();
            if auth == "Bearer stale" {
                (StatusCode::UNAUTHORIZED, "")
            } else {
                (StatusCode::OK, "ok")
            }
        }

        let token = {
            let token_requested = Arc::clone(&token_requested);
            move || async move {
                token_requested.store(true, Ordering::SeqCst);
                r#"{"access_token":"fresh"}"#
            }
        };

        let app = Router::new()
            .route("/token", post(token))
            .route("/data", get(data));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        port
    }

    /// End-to-end test for the reactive re-auth path in `twitch_get_request`.
    ///
    /// The client is seeded with a stale token, so the expected flow is:
    /// 1. `GET /data` with `Bearer stale` -> the mock answers 401
    /// 2. `twitch_get_request` re-authenticates: `POST /token` -> the mock
    ///    answers a fresh token and flags `token_requested`
    /// 3. The GET is retried with `Bearer fresh` -> 200 `ok`
    ///
    /// Both assertions are needed to pin the behavior: a missing retry
    /// fails the body check, a missing re-auth fails the flag check.
    #[tokio::test]
    async fn twitch_401_triggers_reauth_and_retry() {
        let token_requested = Arc::new(AtomicBool::new(false));
        let port = start_mock_twitch(token_requested.clone()).await;

        let crypto_provider = rustls::crypto::ring::default_provider();
        let https = HttpsConnectorBuilder::new()
            .with_provider_and_webpki_roots(crypto_provider)
            .unwrap()
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build();
        let client = Builder::new(TokioExecutor::new()).build(https);

        // Seed a stale token so the first `GET /data` gets a 401.
        let oauth_token: bathbot_model::TwitchOAuthToken =
            serde_json::from_str(r#"{"access_token":"stale"}"#).unwrap();
        let twitch_data = bathbot_model::TwitchData {
            client_id: hyper::header::HeaderValue::from_static("test-client"),
            oauth_token,
        };

        let client = Client {
            client,
            twitch: std::sync::Mutex::new(twitch_data),
            twitch_client_id: "test-client".into(),
            twitch_secret: "test-secret".into(),
            // Point the re-auth POST at the local mock instead of the real
            // Twitch OAuth endpoint.
            twitch_token_url: format!("http://127.0.0.1:{port}/token").into_boxed_str(),
            github_auth: String::new().into_boxed_str(),
            ratelimiters: Ratelimiters::new(),
        };

        let url = format!("http://127.0.0.1:{port}/data");

        let bytes = client
            .make_get_request(url, Site::Twitch)
            .await
            .expect("401 should be transparently retried after re-auth");

        assert_eq!(bytes, bytes::Bytes::from_static(b"ok"));
        assert!(
            token_requested.load(Ordering::SeqCst),
            "the 401 must have triggered a token re-fetch"
        );
    }
}
