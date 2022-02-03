#[derive(Debug, thiserror::Error)]
pub enum PpError {
    #[error("io error")]
    IoError(#[from] tokio::io::Error),
    #[error("failed to download map")]
    MapDownload(#[from] crate::error::MapDownloadError),
    #[error("error while parsing beatmap file")]
    Parse(#[from] rosu_pp::ParseError),
}
