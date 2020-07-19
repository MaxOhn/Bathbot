use crate::roppai::OppaiErr;

use chrono::format::ParseError as ChronoParseError;
use darkredis::Error as RedisError;
use reqwest::Error as ReqwestError;
use rosu::{models::GameMode, OsuError};
use serde_json::Error as SerdeJsonError;
use sqlx::Error as DBError;
use std::{borrow::Cow, error, fmt};
use tokio::io::Error as TokioIOError;
use toml::de::Error as TomlError;
use twilight::gateway::cluster::Error as ClusterError;
use twilight::http::{
    request::channel::message::{
        create_message::CreateMessageError, update_message::UpdateMessageError,
    },
    Error as HttpError,
};

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::Error::Custom(format!("{}", format_args!($($arg)*))))
    };
}

#[macro_export]
macro_rules! format_err {
    ($($arg:tt)*) => {
        $crate::Error::Custom(format!("{}", format_args!($($arg)*)))
    };
}

#[derive(Debug)]
pub enum Error {
    CacheDefrost(&'static str, Box<Error>),
    Command(String, Box<Error>),
    CreateMessage(CreateMessageError),
    ChronoParse(ChronoParseError),
    UpdateMessage(UpdateMessageError),
    Custom(String),
    Database(DBError),
    Fmt(fmt::Error),
    InvalidConfig(TomlError),
    InvalidSession(u64),
    MapDownload(MapDownloadError),
    NoConfig,
    NoLoggingSpec,
    Osu(OsuError),
    PP(PPError),
    Redis(RedisError),
    Serde(SerdeJsonError),
    TwilightHttp(HttpError),
    TwilightCluster(ClusterError),
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::CacheDefrost(reason, e) => {
                write!(f, "error defrosting cache ({}): {}", reason, e)
            }
            Self::Command(cmd, e) => write!(f, "error while processing command `{}`: {}", cmd, e),
            Self::CreateMessage(e) => {
                f.write_str("error while creating message: ")?;
                if let CreateMessageError::EmbedTooLarge { source } = e {
                    source.fmt(f)
                } else {
                    e.fmt(f)
                }
            }
            Self::ChronoParse(e) => write!(f, "chrono parse error: {}", e),
            Self::UpdateMessage(e) => {
                f.write_str("error while updating message: ")?;
                if let UpdateMessageError::EmbedTooLarge { source } = e {
                    source.fmt(f)
                } else {
                    e.fmt(f)
                }
            }
            Self::Custom(e) => e.fmt(f),
            Self::Database(e) => write!(f, "database error occured: {}", e),
            Self::Fmt(e) => write!(f, "fmt error: {}", e),
            Self::InvalidConfig(e) => write!(f, "config file was not in correct format: {}", e),
            Self::InvalidSession(shard) => write!(
                f,
                "gateway invalidated session unrecoverably for shard {}",
                shard
            ),
            Self::MapDownload(e) => write!(f, "error while downloading new map: {}", e),
            Self::NoConfig => f.write_str("config file was not found"),
            Self::NoLoggingSpec => f.write_str("logging config was not found"),
            Self::Osu(e) => write!(f, "osu error: {}", e),
            Self::PP(e) => write!(f, "error while using PPCalculator: {}", e),
            Self::Redis(e) => write!(f, "error while communicating with redis cache: {}", e),
            Self::Serde(e) => write!(f, "serde error: {}", e),
            Self::TwilightHttp(e) => write!(f, "error while making discord request: {}", e),
            Self::TwilightCluster(e) => write!(f, "error occurred on cluster request: {}", e),
        }
    }
}

impl From<CreateMessageError> for Error {
    fn from(e: CreateMessageError) -> Self {
        Error::CreateMessage(e)
    }
}

impl From<ChronoParseError> for Error {
    fn from(e: ChronoParseError) -> Self {
        Error::ChronoParse(e)
    }
}

impl From<DBError> for Error {
    fn from(e: DBError) -> Self {
        Error::Database(e)
    }
}

impl From<fmt::Error> for Error {
    fn from(e: fmt::Error) -> Self {
        Error::Fmt(e)
    }
}
impl From<MapDownloadError> for Error {
    fn from(e: MapDownloadError) -> Self {
        Error::MapDownload(e)
    }
}

impl From<OsuError> for Error {
    fn from(e: OsuError) -> Self {
        Error::Osu(e)
    }
}

impl From<PPError> for Error {
    fn from(e: PPError) -> Self {
        Error::PP(e)
    }
}

impl From<RedisError> for Error {
    fn from(e: RedisError) -> Self {
        Error::Redis(e)
    }
}

impl From<SerdeJsonError> for Error {
    fn from(e: SerdeJsonError) -> Self {
        Error::Serde(e)
    }
}

impl From<HttpError> for Error {
    fn from(e: HttpError) -> Self {
        Error::TwilightHttp(e)
    }
}

impl From<ClusterError> for Error {
    fn from(e: ClusterError) -> Self {
        Error::TwilightCluster(e)
    }
}

impl From<UpdateMessageError> for Error {
    fn from(e: UpdateMessageError) -> Self {
        Error::UpdateMessage(e)
    }
}

#[derive(Debug)]
pub enum PPError {
    CommandLine(String),
    MaxPP(Box<PPError>),
    NoContext(GameMode),
    NoMapId,
    Oppai(OppaiErr),
    PP(Box<PPError>),
    Stars(Box<PPError>),
    Timeout,
}

impl fmt::Display for PPError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::CommandLine(e) => write!(f, "command line error: {}", e),
            Self::MaxPP(e) => write!(f, "error for max pp: {}", e),
            Self::NoContext(m) => write!(f, "missing context for {:?}", m),
            Self::NoMapId => f.write_str("missing map id"),
            Self::Oppai(e) => write!(f, "error while using oppai: {}", e),
            Self::PP(e) => write!(f, "error for pp: {}", e),
            Self::Stars(e) => write!(f, "error for stars: {}", e),
            Self::Timeout => f.write_str("calculation took too long, timed out"),
        }
    }
}

impl From<OppaiErr> for PPError {
    fn from(e: OppaiErr) -> Self {
        Self::Oppai(e)
    }
}

impl error::Error for PPError {}

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

impl error::Error for MapDownloadError {}
