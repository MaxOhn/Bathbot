use crate::util::error::TwitchError;

use handlebars::{RenderError, TemplateError};
use hyper::http::Error as HttpError;
use rosu_v2::prelude::OsuError;
use std::{env::VarError, error::Error as StdError, fmt, io::Error as IoError};

#[derive(Debug)]
pub enum ServerError {
    Http(HttpError),
    Io(IoError),
    MissingEnvVariable,
    Osu(OsuError),
    Render(RenderError),
    Template(TemplateError),
    Twitch(TwitchError),
}

impl From<HttpError> for ServerError {
    fn from(e: HttpError) -> Self {
        Self::Http(e)
    }
}

impl From<IoError> for ServerError {
    fn from(e: IoError) -> Self {
        Self::Io(e)
    }
}

impl From<OsuError> for ServerError {
    fn from(e: OsuError) -> Self {
        Self::Osu(e)
    }
}

impl From<RenderError> for ServerError {
    fn from(e: RenderError) -> Self {
        Self::Render(e)
    }
}

impl From<TemplateError> for ServerError {
    fn from(e: TemplateError) -> Self {
        Self::Template(e)
    }
}

impl From<TwitchError> for ServerError {
    fn from(e: TwitchError) -> Self {
        Self::Twitch(e)
    }
}

impl From<VarError> for ServerError {
    fn from(_: VarError) -> Self {
        Self::MissingEnvVariable
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Http(_) => f.write_str("http error"),
            Self::Io(_) => f.write_str("IO error"),
            Self::MissingEnvVariable => f.write_str("missing an environment variable"),
            Self::Osu(_) => f.write_str("error while interacting with osu!api"),
            Self::Render(_) => f.write_str("failed to render with handlebars"),
            Self::Template(_) => f.write_str("handlebars template error"),
            Self::Twitch(_) => f.write_str("error while interacting with twitch api"),
        }
    }
}

impl StdError for ServerError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Http(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::MissingEnvVariable => None,
            Self::Osu(e) => Some(e),
            Self::Render(e) => Some(e),
            Self::Template(e) => Some(e),
            Self::Twitch(e) => Some(e),
        }
    }
}
