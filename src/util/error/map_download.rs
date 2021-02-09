use reqwest::Error as ReqwestError;
use std::{error::Error as StdError, fmt};
use tokio::io::Error as TokioIOError;

#[derive(Debug)]
pub enum MapDownloadError {
    Content(u32),
    CreateFile(TokioIOError),
    Reqwest(ReqwestError),
}

impl fmt::Display for MapDownloadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Content(map_id) => write!(f, "failed to download {}.osu", map_id),
            Self::CreateFile(_) => f.write_str("could not create file"),
            Self::Reqwest(_) => f.write_str("reqwest error"),
        }
    }
}

impl From<TokioIOError> for MapDownloadError {
    fn from(e: TokioIOError) -> Self {
        Self::CreateFile(e)
    }
}

impl From<ReqwestError> for MapDownloadError {
    fn from(e: ReqwestError) -> Self {
        Self::Reqwest(e)
    }
}

impl StdError for MapDownloadError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Content(_) => None,
            Self::CreateFile(e) => Some(e),
            Self::Reqwest(e) => Some(e),
        }
    }
}
