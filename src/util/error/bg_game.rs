use image::ImageError;
use rosu::{model::GameMode, OsuError};
use std::{error::Error as StdError, fmt};
use tokio::io::Error as TokioIOError;
use tokio::time::Elapsed;

#[derive(Debug)]
pub enum BgGameError {
    Image(ImageError),
    IO(TokioIOError),
    Mode(GameMode),
    NoGame,
    NoMapResult(u32),
    NotStarted,
    Osu(OsuError),
    Restart,
    Stop,
    Timeout,
}

impl fmt::Display for BgGameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Image(_) => f.write_str("image error"),
            Self::IO(_) => f.write_str("IO error"),
            Self::Mode(mode) => write!(f, "background game not available for {}", mode),
            Self::NoGame => f.write_str("no running game in the channel"),
            Self::NoMapResult(id) => write!(f, "api returned no map for mapset id {}", id),
            Self::NotStarted => f.write_str("the game in this channel has not started"),
            Self::Osu(_) => f.write_str("osu error"),
            Self::Restart => f.write_str("could not send restart token"),
            Self::Stop => f.write_str("could not send stop token"),
            Self::Timeout => f.write_str("timed out while waiting for write access"),
        }
    }
}

impl From<ImageError> for BgGameError {
    fn from(e: ImageError) -> Self {
        Self::Image(e)
    }
}

impl From<TokioIOError> for BgGameError {
    fn from(e: TokioIOError) -> Self {
        Self::IO(e)
    }
}

impl From<Elapsed> for BgGameError {
    fn from(_: Elapsed) -> Self {
        Self::Timeout
    }
}

impl StdError for BgGameError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Image(e) => Some(e),
            Self::IO(e) => Some(e),
            Self::Mode(_) => None,
            Self::NoGame => None,
            Self::NoMapResult(_) => None,
            Self::NotStarted => None,
            Self::Osu(e) => Some(e),
            Self::Restart => None,
            Self::Stop => None,
            Self::Timeout => None,
        }
    }
}
