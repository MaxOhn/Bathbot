use std::io::Error as IoError;

use image::ImageError;
use rosu_v2::prelude::{GameMode, OsuError};
use thiserror::Error;
use tokio::time::error::Elapsed;

#[derive(Debug, Error)]
pub enum BgGameError {
    #[error("image error")]
    Image(#[from] ImageError),
    #[error("io error, mapset_id={mapset_id}")]
    Io {
        #[source]
        source: IoError,
        mapset_id: u32,
    },
    #[error("creating subimage failed")]
    IoSubimage(#[from] IoError),
    #[error("background game not available for {0}")]
    Mode(GameMode),
    #[error("no running game in the channel")]
    NoGame,
    #[error("osu error")]
    Osu(#[from] OsuError),
    #[error("could not send restart token")]
    RestartToken,
    #[error("could not send stop token")]
    StopToken,
    #[error("timed out while waiting for write access")]
    Timeout,
}

impl From<Elapsed> for BgGameError {
    fn from(_: Elapsed) -> Self {
        Self::Timeout
    }
}
