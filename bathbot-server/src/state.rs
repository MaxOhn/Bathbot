use std::{path::PathBuf, sync::Arc};

use eyre::{ContextCompat, Result, WrapErr};
use handlebars::Handlebars;
use hyper::{
    client::{connect::dns::GaiResolver, HttpConnector},
    Body, Client,
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use prometheus::Registry;

use crate::standby::AuthenticationStandby;

pub struct AppState {
    pub(crate) client: Client<HttpsConnector<HttpConnector<GaiResolver>>, Body>,
    pub(crate) handlebars: Handlebars<'static>,
    pub(crate) metrics: Registry,
    pub(crate) osu_client_id: u64,
    pub(crate) osu_client_secret: String,
    pub(crate) osu_redirect: String,
    pub(crate) twitch_client_id: String,
    pub(crate) twitch_token: String,
    pub(crate) twitch_redirect: String,
    pub(crate) standby: Arc<AuthenticationStandby>,
}

#[derive(Default)]
pub struct AppStateBuilder {
    website_path: Option<PathBuf>,
    metrics: Option<Registry>,
    osu_client_id: Option<u64>,
    osu_client_secret: Option<String>,
    osu_redirect: Option<String>,
    twitch_client_id: Option<String>,
    twitch_token: Option<String>,
    twitch_redirect: Option<String>,
}

impl AppStateBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> Result<AppState> {
        let connector = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .build();

        let mut handlebars = Handlebars::new();
        let mut path = self.website_path.wrap_err("missing `website_path`")?;
        path.push("auth.hbs");

        handlebars
            .register_template_file("auth", path)
            .wrap_err("failed to register auth template to handlebars")?;

        Ok(AppState {
            client: Client::builder().build(connector),
            handlebars,
            metrics: self.metrics.wrap_err("missing `metrics`")?,
            osu_client_id: self.osu_client_id.wrap_err("missing `osu_client_id`")?,
            osu_client_secret: self
                .osu_client_secret
                .wrap_err("missing `osu_client_secret`")?,
            osu_redirect: self.osu_redirect.wrap_err("missing `osu_redirect`")?,
            twitch_client_id: self
                .twitch_client_id
                .wrap_err("missing `twitch_client_id`")?,
            twitch_token: self.twitch_token.wrap_err("missing `twitch_token`")?,
            twitch_redirect: self.twitch_redirect.wrap_err("missing `twitch_redirect`")?,
            standby: Arc::new(AuthenticationStandby::new()),
        })
    }

    pub fn website(self, path: impl Into<PathBuf>) -> Self {
        Self {
            website_path: Some(path.into()),
            ..self
        }
    }

    pub fn metrics(self, metrics: Registry) -> Self {
        Self {
            metrics: Some(metrics),
            ..self
        }
    }

    pub fn osu_client_id(self, client_id: u64) -> Self {
        Self {
            osu_client_id: Some(client_id),
            ..self
        }
    }

    pub fn osu_client_secret(self, client_secret: impl Into<String>) -> Self {
        Self {
            osu_client_secret: Some(client_secret.into()),
            ..self
        }
    }

    pub fn osu_redirect(self, url: impl Into<String>) -> Self {
        Self {
            osu_redirect: Some(url.into()),
            ..self
        }
    }

    pub fn twitch_client_id(self, client_id: impl Into<String>) -> Self {
        Self {
            twitch_client_id: Some(client_id.into()),
            ..self
        }
    }

    pub fn twitch_token(self, token: impl Into<String>) -> Self {
        Self {
            twitch_token: Some(token.into()),
            ..self
        }
    }

    pub fn twitch_redirect(self, url: impl Into<String>) -> Self {
        Self {
            twitch_redirect: Some(url.into()),
            ..self
        }
    }
}
