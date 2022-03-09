#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("custom client error")]
    CustomClient(#[from] crate::custom_client::CustomClientError),
    #[error("http error")]
    Http(#[from] hyper::http::Error),
    #[error("hyper error")]
    Hyper(#[from] hyper::Error),
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("no twitch user provided by api after authorization")]
    NoTwitchUser,
    #[error("osu error")]
    Osu(#[from] rosu_v2::error::OsuError),
    #[error("failed to render with handlebars")]
    Render(#[from] handlebars::RenderError),
    #[error("handlebars template error")]
    Template(#[from] handlebars::TemplateError),
}
