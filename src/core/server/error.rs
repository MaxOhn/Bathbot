use std::env::VarError;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("http error")]
    Http(#[from] hyper::http::Error),
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("missing an environment variable")]
    MissingEnvVariable,
    #[error("osu error")]
    Osu(#[from] rosu_v2::error::OsuError),
    #[error("failed to render with handlebars")]
    Render(#[from] handlebars::RenderError),
    #[error("handlebars template error")]
    Template(#[from] handlebars::TemplateError),
    #[error("twitch error")]
    Twitch(#[from] crate::error::TwitchError),
}

impl From<VarError> for ServerError {
    fn from(_: VarError) -> Self {
        Self::MissingEnvVariable
    }
}
