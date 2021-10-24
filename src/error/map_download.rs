use thiserror::Error;

#[derive(Debug, Error)]
pub enum MapDownloadError {
    #[error("failed to download {0}.osu")]
    Content(u32),
    #[error("could not create file")]
    CreateFile(#[from] tokio::io::Error),
    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),
}
