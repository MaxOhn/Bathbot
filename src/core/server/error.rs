use crate::util::error::TwitchError;

use hyper::http::Error as HttpError;
use rosu_v2::prelude::OsuError;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum ServerError {
    Http(HttpError),
    Osu(OsuError),
    Twitch(TwitchError),
}

impl From<HttpError> for ServerError {
    fn from(e: HttpError) -> Self {
        Self::Http(e)
    }
}

impl From<OsuError> for ServerError {
    fn from(e: OsuError) -> Self {
        Self::Osu(e)
    }
}

impl From<TwitchError> for ServerError {
    fn from(e: TwitchError) -> Self {
        Self::Twitch(e)
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Http(_) => f.write_str("http error"),
            Self::Osu(_) => f.write_str("error while interacting with osu!api"),
            Self::Twitch(_) => f.write_str("error while interacting with twitch api"),
        }
    }
}

impl StdError for ServerError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Http(e) => Some(e),
            Self::Osu(e) => Some(e),
            Self::Twitch(e) => Some(e),
        }
    }
}
