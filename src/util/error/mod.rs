mod bg_game;
mod custom_client;
mod map_download;
mod pp;
mod twitch;

pub use bg_game::BgGameError;
pub use custom_client::CustomClientError;
pub use map_download::MapDownloadError;
pub use pp::PPError;
use twilight_model::application::interaction::{ApplicationCommand, MessageComponentInteraction};
pub use twitch::TwitchError;

use chrono::format::ParseError as ChronoParseError;
use deadpool_redis::{redis::RedisError, CreatePoolError};
use image::ImageError;
use plotters::drawing::DrawingAreaErrorKind as DrawingError;
use reqwest::Error as ReqwestError;
use rosu_v2::error::OsuError;
use serde_json::Error as SerdeJsonError;
use sqlx::Error as DBError;
use std::{
    error::Error as StdError,
    fmt::{Display, Error as FmtError, Formatter, Result as FmtResult},
    io::Error as IOError,
};
use toml::de::Error as TomlError;
use twilight_gateway::cluster::ClusterCommandError;
use twilight_http::{
    request::{
        application::{
            interaction::update_original_response::UpdateOriginalResponseError, InteractionError,
        },
        channel::message::{
            create_message::CreateMessageError, update_message::UpdateMessageError,
        },
    },
    response::DeserializeBodyError,
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
pub enum InvalidHelpState {
    UnknownCommand,
    MissingEmbed,
    MissingTitle,
}

impl Display for InvalidHelpState {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::UnknownCommand => f.write_str("unknown command"),
            Self::MissingEmbed => f.write_str("missing embed"),
            Self::MissingTitle => f.write_str("missing embed title"),
        }
    }
}

impl StdError for InvalidHelpState {}

#[derive(Debug)]
pub enum Error {
    Authority(Box<Error>),
    BgGame(BgGameError),
    CreateMessage(CreateMessageError),
    ChronoParse(ChronoParseError),
    Command(Box<Error>, String),
    CreateRedisPool(CreatePoolError),
    Custom(String),
    CustomClient(CustomClientError),
    Database(DBError),
    Fmt(FmtError),
    Image(ImageError),
    Interaction(InteractionError),
    InvalidCommandOptions,
    InvalidHelpState(InvalidHelpState),
    InvalidConfig(TomlError),
    IO(IOError),
    MapDownload(MapDownloadError),
    MissingInteractionAuthor,
    NoConfig,
    NoLoggingSpec,
    Osu(OsuError),
    Plotter(String),
    PP(PPError),
    Redis(RedisError),
    Reqwest(ReqwestError),
    Serde(SerdeJsonError),
    TwilightCluster(ClusterCommandError),
    TwilightDeserialize(DeserializeBodyError),
    TwilightHttp(HttpError),
    Twitch(TwitchError),
    UnexpectedCommandOption {
        cmd: &'static str,
        kind: &'static str,
        name: String,
    },
    UnknownMessageComponent {
        component: Box<MessageComponentInteraction>,
    },
    UnknownSlashCommand {
        name: String,
        command: Box<ApplicationCommand>,
    },
    UpdateMessage(UpdateMessageError),
    UpdateOriginalResponse(UpdateOriginalResponseError),
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Authority(e) => Some(e),
            Self::BgGame(e) => Some(e),
            Self::CreateMessage(e) => Some(e),
            Self::ChronoParse(e) => Some(e),
            Self::Command(e, _) => Some(e),
            Self::CreateRedisPool(e) => Some(e),
            Self::Custom(_) => None,
            Self::CustomClient(e) => Some(e),
            Self::Database(e) => Some(e),
            Self::Fmt(e) => Some(e),
            Self::Image(e) => Some(e),
            Self::Interaction(e) => Some(e),
            Self::InvalidCommandOptions => None,
            Self::InvalidConfig(e) => Some(e),
            Self::InvalidHelpState(e) => Some(e),
            Self::IO(e) => Some(e),
            Self::MapDownload(e) => Some(e),
            Self::MissingInteractionAuthor => None,
            Self::NoConfig => None,
            Self::NoLoggingSpec => None,
            Self::Osu(e) => Some(e),
            Self::Plotter(_) => None,
            Self::PP(e) => Some(e),
            Self::Redis(e) => Some(e),
            Self::Reqwest(e) => Some(e),
            Self::Serde(e) => Some(e),
            Self::TwilightCluster(e) => Some(e),
            Self::TwilightDeserialize(e) => Some(e),
            Self::TwilightHttp(e) => Some(e),
            Self::Twitch(e) => Some(e),
            Self::UnexpectedCommandOption { .. } => None,
            Self::UnknownMessageComponent { .. } => None,
            Self::UnknownSlashCommand { .. } => None,
            Self::UpdateMessage(e) => Some(e),
            Self::UpdateOriginalResponse(e) => Some(e),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Self::Authority(_) => f.write_str("error while checking authorty status"),
            Self::BgGame(_) => f.write_str("background game error"),
            Self::CreateMessage(_) => f.write_str("error while creating message"),
            Self::ChronoParse(_) => f.write_str("chrono parse error"),
            Self::Command(_, cmd) => write!(f, "command error: {}", cmd),
            Self::CreateRedisPool(_) => f.write_str("failed to create redis pool"),
            Self::Custom(e) => e.fmt(f),
            Self::CustomClient(_) => f.write_str("custom client error"),
            Self::Database(_) => f.write_str("database error"),
            Self::Fmt(_) => f.write_str("fmt error"),
            Self::Image(_) => f.write_str("image error"),
            Self::Interaction(_) => f.write_str("interaction error"),
            Self::InvalidCommandOptions => f.write_str("received invalid options for command"),
            Self::InvalidConfig(_) => f.write_str("config file was not in correct format"),
            Self::InvalidHelpState(_) => f.write_str("invalid help state"),
            Self::IO(_) => f.write_str("io error"),
            Self::MapDownload(_) => f.write_str("error while downloading new map"),
            Self::MissingInteractionAuthor => {
                f.write_str("interaction contained neither member nor user")
            }
            Self::NoConfig => f.write_str("config file was not found"),
            Self::NoLoggingSpec => f.write_str("logging config was not found"),
            Self::Osu(_) => f.write_str("osu error"),
            Self::Plotter(e) => write!(f, "plotter error: {}", e),
            Self::PP(_) => f.write_str("error while using PPCalculator"),
            Self::Redis(_) => f.write_str("error while communicating with redis cache"),
            Self::Reqwest(_) => f.write_str("reqwest error"),
            Self::Serde(_) => f.write_str("serde error"),
            Self::TwilightCluster(_) => f.write_str("error occurred on cluster request"),
            Self::TwilightDeserialize(_) => f.write_str("twilight failed to deserialize response"),
            Self::TwilightHttp(_) => f.write_str("error while making discord request"),
            Self::Twitch(_) => f.write_str("twitch error"),
            Self::UnexpectedCommandOption { cmd, kind, name } => write!(
                f,
                "unexpected {} option for slash command `{}`: `{}`",
                kind, cmd, name
            ),
            Self::UnknownMessageComponent { component } => {
                write!(f, "unknown message component: {:#?}", component)
            }
            Self::UnknownSlashCommand { name, command } => {
                write!(f, "unknown slash command `{}`: {:#?}", name, command)
            }
            Self::UpdateMessage(_) => f.write_str("error while updating message"),
            Self::UpdateOriginalResponse(_) => f.write_str("update original response error"),
        }
    }
}

impl From<BgGameError> for Error {
    fn from(e: BgGameError) -> Self {
        Error::BgGame(e)
    }
}

impl From<ChronoParseError> for Error {
    fn from(e: ChronoParseError) -> Self {
        Error::ChronoParse(e)
    }
}

impl From<ClusterCommandError> for Error {
    fn from(e: ClusterCommandError) -> Self {
        Error::TwilightCluster(e)
    }
}

impl From<CreateMessageError> for Error {
    fn from(e: CreateMessageError) -> Self {
        Error::CreateMessage(e)
    }
}

impl From<CreatePoolError> for Error {
    fn from(e: CreatePoolError) -> Self {
        Error::CreateRedisPool(e)
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

impl From<DeserializeBodyError> for Error {
    fn from(e: DeserializeBodyError) -> Self {
        Error::TwilightDeserialize(e)
    }
}

impl<T: StdError + Send + Sync> From<DrawingError<T>> for Error {
    fn from(e: DrawingError<T>) -> Self {
        Error::Plotter(e.to_string())
    }
}

impl From<FmtError> for Error {
    fn from(e: FmtError) -> Self {
        Error::Fmt(e)
    }
}

impl From<HttpError> for Error {
    fn from(e: HttpError) -> Self {
        Error::TwilightHttp(e)
    }
}

impl From<ImageError> for Error {
    fn from(e: ImageError) -> Self {
        Error::Image(e)
    }
}

impl From<InteractionError> for Error {
    fn from(e: InteractionError) -> Self {
        Error::Interaction(e)
    }
}

impl From<InvalidHelpState> for Error {
    fn from(e: InvalidHelpState) -> Self {
        Error::InvalidHelpState(e)
    }
}

impl From<IOError> for Error {
    fn from(e: IOError) -> Self {
        Error::IO(e)
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

impl From<UpdateOriginalResponseError> for Error {
    fn from(e: UpdateOriginalResponseError) -> Self {
        Error::UpdateOriginalResponse(e)
    }
}
