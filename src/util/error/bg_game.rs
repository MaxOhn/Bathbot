use image::ImageError;
use rosu_v2::prelude::{GameMode, OsuError};
use std::{error::Error as StdError, fmt};
use tokio::io::Error as TokioIOError;
use tokio::time::error::Elapsed;

#[derive(Debug)]
pub enum BgGameError {
    Image(ImageError),
    IO(TokioIOError, u32),
    Mode(GameMode),
    NoGame,
    NotStarted,
    Osu(OsuError),
    RestartToken,
    StopToken,
    Timeout,
}

impl fmt::Display for BgGameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Image(_) => f.write_str("image error"),
            Self::IO(_, mapset_id) => write!(f, "IO error | mapset_id={}", mapset_id),
            Self::Mode(mode) => write!(f, "background game not available for {}", mode),
            Self::NoGame => f.write_str("no running game in the channel"),
            Self::NotStarted => f.write_str("the game in this channel has not started"),
            Self::Osu(_) => f.write_str("osu error"),
            Self::RestartToken => f.write_str("could not send restart token"),
            Self::StopToken => f.write_str("could not send stop token"),
            Self::Timeout => f.write_str("timed out while waiting for write access"),
        }
    }
}

impl From<ImageError> for BgGameError {
    fn from(e: ImageError) -> Self {
        Self::Image(e)
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
            Self::IO(e, _) => Some(e),
            Self::Mode(_) => None,
            Self::NoGame => None,
            Self::NotStarted => None,
            Self::Osu(e) => Some(e),
            Self::RestartToken => None,
            Self::StopToken => None,
            Self::Timeout => None,
        }
    }
}
