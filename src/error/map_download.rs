#[derive(Debug, thiserror::Error)]
pub enum MapDownloadError {
    #[error("reached retry limit and still failed to download {0}.osu")]
    RetryLimit(u32),
    #[error("could not create file")]
    CreateFile(#[from] tokio::io::Error),
    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),
}
