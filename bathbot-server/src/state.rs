use std::{path::PathBuf, sync::Arc};

use axum::body::Bytes;
use bathbot_util::MetricsReader;
use eyre::{Result, WrapErr};
use handlebars::Handlebars;
use http_body_util::Empty;
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Builder, Client as HyperClient, connect::HttpConnector},
    rt::TokioExecutor,
};
use metrics::describe_histogram;
use metrics_exporter_prometheus::PrometheusHandle;

use crate::standby::AuthenticationStandby;

pub struct AppState {
    pub client: HyperClient<HttpsConnector<HttpConnector>, Empty<Bytes>>,
    pub handlebars: Handlebars<'static>,
    pub prometheus: PrometheusHandle,
    pub metrics_reader: MetricsReader,
    pub osu_client_id: u64,
    pub osu_client_secret: Box<str>,
    pub twitch_client_id: Box<str>,
    pub twitch_token: Box<str>,
    pub redirect_base: Box<str>,
    pub standby: Arc<AuthenticationStandby>,
}

pub struct AppStateBuilder {
    pub website_path: PathBuf,
    pub prometheus: PrometheusHandle,
    pub metrics_reader: MetricsReader,
    pub osu_client_id: u64,
    pub osu_client_secret: String,
    pub twitch_client_id: String,
    pub twitch_token: String,
    pub redirect_base: String,
}

impl AppStateBuilder {
    pub(crate) fn build(self, standby: Arc<AuthenticationStandby>) -> Result<(AppState, PathBuf)> {
        let Self {
            website_path,
            prometheus,
            metrics_reader,
            osu_client_id,
            osu_client_secret,
            twitch_client_id,
            twitch_token,
            redirect_base,
        } = self;

        let crypto_provider = rustls::crypto::ring::default_provider();

        let https = HttpsConnectorBuilder::new()
            .with_provider_and_webpki_roots(crypto_provider)
            .wrap_err("Failed to configure https connector")?
            .https_only()
            .enable_http2()
            .build();

        let client = Builder::new(TokioExecutor::new())
            .http2_only(true)
            .build(https);

        let mut handlebars = Handlebars::new();
        let mut path = website_path.clone();
        path.push("assets/auth/auth.hbs");

        handlebars
            .register_template_file("auth", &path)
            .wrap_err_with(|| {
                format!("Failed to register auth template at `{path:?}` to handlebars")
            })?;

        describe_histogram!(
            "server_response_time",
            "Response time for requests to the server"
        );

        let state = AppState {
            client,
            handlebars,
            prometheus,
            metrics_reader,
            osu_client_id,
            osu_client_secret: osu_client_secret.into_boxed_str(),
            twitch_client_id: twitch_client_id.into_boxed_str(),
            twitch_token: twitch_token.into_boxed_str(),
            redirect_base: redirect_base.into_boxed_str(),
            standby,
        };

        Ok((state, website_path))
    }
}
