use rosu_v2::prelude::{GameMode, OsuError};
use tokio::time::error::Elapsed;

#[derive(Debug, thiserror::Error)]
pub enum InvalidBgState {
    #[error("missing embed")]
    MissingEmbed,
}

#[derive(Debug, thiserror::Error)]
pub enum BgGameError {
    #[error("image error")]
    Image(#[from] image::ImageError),
    #[error("io error, mapset_id={mapset_id}")]
    Io {
        #[source]
        source: std::io::Error,
        mapset_id: u32,
    },
    #[error("creating subimage failed")]
    IoSubimage(#[from] std::io::Error),
    #[error("background game not available for {0}")]
    Mode(GameMode),
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
