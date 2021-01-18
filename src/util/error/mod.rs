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
use twilight_embed_builder::builder::{
    EmbedBuildError, EmbedColorError, EmbedDescriptionError, EmbedTitleError,
};
use twilight_gateway::cluster::ClusterCommandError;
use twilight_http::{
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
    Authority(Box<Error>),
    BgGame(BgGameError),
    CacheDefrost(&'static str, Box<Error>),
    CreateMessage(CreateMessageError),
    ChronoParse(ChronoParseError),
    Command(Box<Error>, String),
    Custom(String),
    CustomClient(CustomClientError),
    Database(DBError),
    Embed(EmbedBuildError),
    EmbedColor(EmbedColorError),
    EmbedDescription(EmbedDescriptionError),
    EmbedTitle(EmbedTitleError),
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
    TwilightCluster(ClusterCommandError),
    TwilightHttp(HttpError),
    Twitch(TwitchError),
    UpdateMessage(UpdateMessageError),
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Authority(e) => Some(e),
            Self::BgGame(e) => Some(e),
            Self::CacheDefrost(_, e) => Some(e),
            Self::CreateMessage(e) => Some(e),
            Self::ChronoParse(e) => Some(e),
            Self::Command(e, _) => Some(e),
            Self::Custom(_) => None,
            Self::CustomClient(e) => Some(e),
            Self::Database(e) => Some(e),
            Self::Embed(e) => Some(e),
            Self::EmbedColor(e) => Some(e),
            Self::EmbedDescription(e) => Some(e),
            Self::EmbedTitle(e) => Some(e),
            Self::Fmt(e) => Some(e),
            Self::Image(e) => Some(e),
            Self::InvalidConfig(e) => Some(e),
            Self::InvalidSession(_) => None,
            Self::IO(e) => Some(e),
            Self::MapDownload(e) => Some(e),
            Self::NoConfig => None,
            Self::NoLoggingSpec => None,
            Self::Osu(e) => Some(e),
            Self::Plotter(_) => None,
            Self::PP(e) => Some(e),
            Self::Redis(e) => Some(e),
            Self::Reqwest(e) => Some(e),
            Self::Serde(e) => Some(e),
            Self::TwilightCluster(e) => Some(e),
            Self::TwilightHttp(e) => Some(e),
            Self::Twitch(e) => Some(e),
            Self::UpdateMessage(e) => Some(e),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Authority(_) => f.write_str("error while checking authorty status"),
            Self::BgGame(_) => f.write_str("background game error"),
            Self::CacheDefrost(reason, _) => write!(f, "error defrosting cache: {}", reason),
            Self::CreateMessage(_) => f.write_str("error while creating message"),
            Self::ChronoParse(_) => f.write_str("chrono parse error"),
            Self::Command(_, cmd) => write!(f, "command error: {}", cmd),
            Self::Custom(e) => e.fmt(f),
            Self::CustomClient(_) => f.write_str("custom client error"),
            Self::Database(_) => f.write_str("database error"),
            Self::Embed(_) => f.write_str("error while building embed"),
            Self::EmbedColor(_) => f.write_str("embed color error"),
            Self::EmbedDescription(_) => f.write_str("embed description error"),
            Self::EmbedTitle(_) => f.write_str("embed title error"),
            Self::Fmt(_) => f.write_str("fmt error"),
            Self::Image(_) => f.write_str("image error"),
            Self::InvalidConfig(_) => f.write_str("config file was not in correct format"),
            Self::InvalidSession(shard) => write!(
                f,
                "gateway invalidated session unrecoverably for shard {}",
                shard
            ),
            Self::IO(_) => f.write_str("io error"),
            Self::MapDownload(_) => f.write_str("error while downloading new map"),
            Self::NoConfig => f.write_str("config file was not found"),
            Self::NoLoggingSpec => f.write_str("logging config was not found"),
            Self::Osu(_) => f.write_str("osu error"),
            Self::Plotter(e) => write!(f, "plotter error: {}", e),
            Self::PP(_) => f.write_str("error while using PPCalculator"),
            Self::Redis(_) => f.write_str("error while communicating with redis cache"),
            Self::Reqwest(_) => f.write_str("reqwest error"),
            Self::Serde(_) => f.write_str("serde error"),
            Self::TwilightHttp(_) => f.write_str("error while making discord request"),
            Self::TwilightCluster(_) => f.write_str("error occurred on cluster request"),
            Self::Twitch(_) => f.write_str("twitch error"),
            Self::UpdateMessage(_) => f.write_str("error while updating message"),
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

impl From<EmbedBuildError> for Error {
    fn from(e: EmbedBuildError) -> Self {
        Error::Embed(e)
    }
}

impl From<EmbedColorError> for Error {
    fn from(e: EmbedColorError) -> Self {
        Error::EmbedColor(e)
    }
}

impl From<EmbedDescriptionError> for Error {
    fn from(e: EmbedDescriptionError) -> Self {
        Error::EmbedDescription(e)
    }
}

impl From<EmbedTitleError> for Error {
    fn from(e: EmbedTitleError) -> Self {
        Error::EmbedTitle(e)
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

impl From<ClusterCommandError> for Error {
    fn from(e: ClusterCommandError) -> Self {
        Error::TwilightCluster(e)
    }
}

impl From<HttpError> for Error {
    fn from(e: HttpError) -> Self {
        Error::TwilightHttp(e)
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
