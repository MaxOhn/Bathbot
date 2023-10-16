use std::{path::PathBuf, sync::Arc};

use bathbot_util::MetricsReader;
use eyre::{Result, WrapErr};
use handlebars::Handlebars;
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    Body, Client,
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use metrics::describe_histogram;
use metrics_exporter_prometheus::PrometheusHandle;

use crate::standby::AuthenticationStandby;

pub struct AppState {
    pub client: Client<HttpsConnector<HttpConnector<GaiResolver>>, Body>,
    pub handlebars: Handlebars<'static>,
    pub prometheus: PrometheusHandle,
    pub metrics_reader: MetricsReader,
    pub osu_client_id: u64,
    pub osu_client_secret: String, // TODO: box strings?
    pub twitch_client_id: String,
    pub twitch_token: String,
    pub redirect_base: String,
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

        let connector = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .build();

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
            client: Client::builder().build(connector),
            handlebars,
            prometheus,
            metrics_reader,
            osu_client_id,
            osu_client_secret,
            twitch_client_id,
            twitch_token,
            redirect_base,
            standby,
        };

        Ok((state, website_path))
    }
}
