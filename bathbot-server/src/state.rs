use std::{path::PathBuf, sync::Arc};

use eyre::{Result, WrapErr};
use handlebars::Handlebars;
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    Body, Client,
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use prometheus::{
    histogram_opts, opts, HistogramVec, IntCounterVec, IntGauge, Registry, DEFAULT_BUCKETS,
};

use crate::standby::AuthenticationStandby;

pub struct AppState {
    pub client: Client<HttpsConnector<HttpConnector<GaiResolver>>, Body>,
    pub handlebars: Handlebars<'static>,
    pub metrics: AppMetrics,
    pub osu_client_id: u64,
    pub osu_client_secret: String,
    pub twitch_client_id: String,
    pub twitch_token: String,
    pub redirect_base: String,
    pub standby: Arc<AuthenticationStandby>,
}

pub struct AppMetrics {
    pub registry: Registry,
    pub guild_counter: IntGauge,
    pub request_count: IntCounterVec,
    pub response_time: HistogramVec,
}

pub struct AppStateBuilder {
    pub website_path: PathBuf,
    pub metrics: Registry,
    pub guild_counter: IntGauge,
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
            metrics,
            guild_counter,
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
                format!("failed to register auth template at `{path:?}` to handlebars")
            })?;

        let opts = opts!("requests_total", "Requests total");
        let request_count = IntCounterVec::new(opts, &["method", "path", "status"])
            .wrap_err("failed to create request count")?;

        let opts = histogram_opts!(
            "response_time_seconds",
            "Response times",
            DEFAULT_BUCKETS.to_vec()
        );
        let response_time = HistogramVec::new(opts, &["method", "path", "status"])
            .wrap_err("failed to create response time")?;

        metrics
            .register(Box::new(request_count.clone()))
            .wrap_err("failed to register request count")?;

        metrics
            .register(Box::new(response_time.clone()))
            .wrap_err("failed to register response time")?;

        let metrics = AppMetrics {
            registry: metrics,
            guild_counter,
            request_count,
            response_time,
        };

        let state = AppState {
            client: Client::builder().build(connector),
            handlebars,
            metrics,
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
