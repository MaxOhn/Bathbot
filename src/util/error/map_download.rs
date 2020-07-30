use reqwest::Error as ReqwestError;
use std::{error::Error as StdError, fmt};
use tokio::io::Error as TokioIOError;

#[derive(Debug)]
pub enum MapDownloadError {
    CreateFile(TokioIOError),
    NoEnv,
    Reqwest(ReqwestError),
}

impl fmt::Display for MapDownloadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::CreateFile(e) => write!(f, "could not create file: {}", e),
            Self::NoEnv => f.write_str("no `BEATMAP_PATH` variable in .env file"),
            Self::Reqwest(e) => write!(f, "reqwest error: {}", e),
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

impl StdError for MapDownloadError {}
