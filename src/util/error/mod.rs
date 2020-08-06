mod bg_game;
mod custom_client;
mod map_download;
mod pp;
mod twitch;

pub use bg_game::BgGameError;
pub use custom_client::CustomClientError;
pub use map_download::MapDownloadError;
pub use pp::PPError;
pub use twitch::TwitchError;

use chrono::format::ParseError as ChronoParseError;
use darkredis::Error as RedisError;
use image::ImageError;
use plotters::drawing::DrawingAreaErrorKind as DrawingError;
use reqwest::Error as ReqwestError;
use rosu::OsuError;
use serde_json::Error as SerdeJsonError;
use sqlx::Error as DBError;
use std::{error::Error as StdError, fmt, io::Error as IOError};
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
    BgGame(BgGameError),
    CacheDefrost(&'static str, Box<Error>),
    CreateMessage(CreateMessageError),
    ChronoParse(ChronoParseError),
    Custom(String),
    CustomClient(CustomClientError),
    Database(DBError),
    Fmt(fmt::Error),
    Image(ImageError),
    InvalidConfig(TomlError),
    InvalidSession(u64),
    IO(IOError),
    MapDownload(MapDownloadError),
    NoConfig,
    NoLoggingSpec,
    Osu(OsuError),
    Plotter(String),
    PP(PPError),
    Redis(RedisError),
    Reqwest(ReqwestError),
    Serde(SerdeJsonError),
    TwilightHttp(HttpError),
    TwilightCluster(ClusterError),
    Twitch(TwitchError),
    UpdateMessage(UpdateMessageError),
}

impl StdError for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::BgGame(e) => write!(f, "background game error: {}", e),
            Self::CacheDefrost(reason, e) => {
                write!(f, "error defrosting cache ({}): {}", reason, e)
            }
            Self::CreateMessage(e) => {
                f.write_str("error while creating message: ")?;
                if let CreateMessageError::EmbedTooLarge { source } = e {
                    source.fmt(f)
                } else {
                    e.fmt(f)
                }
            }
            Self::ChronoParse(e) => write!(f, "chrono parse error: {}", e),
            Self::Custom(e) => e.fmt(f),
            Self::CustomClient(e) => write!(f, "custom client error: {}", e),
            Self::Database(e) => write!(f, "database error occured: {}", e),
            Self::Fmt(e) => write!(f, "fmt error: {}", e),
            Self::Image(e) => write!(f, "image error: {}", e),
            Self::InvalidConfig(e) => write!(f, "config file was not in correct format: {}", e),
            Self::InvalidSession(shard) => write!(
                f,
                "gateway invalidated session unrecoverably for shard {}",
                shard
            ),
            Self::IO(e) => write!(f, "io error: {}", e),
            Self::MapDownload(e) => write!(f, "error while downloading new map: {}", e),
            Self::NoConfig => f.write_str("config file was not found"),
            Self::NoLoggingSpec => f.write_str("logging config was not found"),
            Self::Osu(e) => write!(f, "osu error: {}", e),
            Self::Plotter(e) => write!(f, "plotter error: {}", e),
            Self::PP(e) => write!(f, "error while using PPCalculator: {}", e),
            Self::Redis(e) => write!(f, "error while communicating with redis cache: {}", e),
            Self::Reqwest(e) => write!(f, "reqwest error: {}", e),
            Self::Serde(e) => write!(f, "serde error: {}", e),
            Self::TwilightHttp(e) => write!(f, "error while making discord request: {}", e),
            Self::TwilightCluster(e) => write!(f, "error occurred on cluster request: {}", e),
            Self::Twitch(e) => write!(f, "twitch error: {}", e),
            Self::UpdateMessage(e) => {
                f.write_str("error while updating message: ")?;
                if let UpdateMessageError::EmbedTooLarge { source } = e {
                    source.fmt(f)
                } else {
                    e.fmt(f)
                }
            }
        }
    }
}

impl From<BgGameError> for Error {
    fn from(e: BgGameError) -> Self {
        Error::BgGame(e)
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

impl From<CustomClientError> for Error {
    fn from(e: CustomClientError) -> Self {
        Error::CustomClient(e)
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

impl From<ImageError> for Error {
    fn from(e: ImageError) -> Self {
        Error::Image(e)
    }
}

impl From<MapDownloadError> for Error {
    fn from(e: MapDownloadError) -> Self {
        Error::MapDownload(e)
    }
}

impl From<IOError> for Error {
    fn from(e: IOError) -> Self {
        Error::IO(e)
    }
}

impl From<OsuError> for Error {
    fn from(e: OsuError) -> Self {
        Error::Osu(e)
    }
}

impl<T: StdError + Send + Sync> From<DrawingError<T>> for Error {
    fn from(e: DrawingError<T>) -> Self {
        Error::Plotter(e.to_string())
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

impl From<ReqwestError> for Error {
    fn from(e: ReqwestError) -> Self {
        Error::Reqwest(e)
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

impl From<TwitchError> for Error {
    fn from(e: TwitchError) -> Self {
        Error::Twitch(e)
    }
}

impl From<UpdateMessageError> for Error {
    fn from(e: UpdateMessageError) -> Self {
        Error::UpdateMessage(e)
    }
}
