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

use plotters::drawing::DrawingAreaErrorKind;
use twilight_http::request::{
    application::{
        interaction::update_original_response::UpdateOriginalResponseError, InteractionError,
    },
    channel::message::{create_message::CreateMessageError, update_message::UpdateMessageError},
};
use twilight_model::application::interaction::{ApplicationCommand, MessageComponentInteraction};

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::Error::Custom(format!("{}", format_args!($($arg)*))))
    };
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("error while checking authority status")]
    Authority(#[source] Box<Error>),
    #[error("background game error")]
    BgGame(#[from] bg_game::BgGameError),
    #[error("cache error")]
    Cache(#[from] bathbot_cache::CacheError),
    #[error("serde cbor error")]
    Cbor(#[from] serde_cbor::Error),
    #[error("error occured on cluster request")]
    ClusterCommand(#[from] twilight_gateway::cluster::ClusterCommandError),
    #[error("failed to start cluster")]
    ClusterStart(#[from] twilight_gateway::cluster::ClusterStartError),
    #[error("error while creating message")]
    CreateMessage(#[from] CreateMessageError),
    #[error("chrono parse error")]
    ChronoParse(#[from] chrono::format::ParseError),
    #[error("command error: {1}")]
    Command(#[source] Box<Error>, String),
    #[error("failed to create redis pool")]
    CreateRedisPool(#[from] deadpool_redis::CreatePoolError),
    #[error("{0}")]
    Custom(String),
    #[error("custom client error")]
    CustomClient(#[from] custom_client::CustomClientError),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("fmt error")]
    Fmt(#[from] std::fmt::Error),
    #[error("image error")]
    Image(#[from] image::ImageError),
    #[error("interaction error")]
    Interaction(#[from] InteractionError),
    #[error("received invalid options for command")]
    InvalidCommandOptions,
    #[error("config file was not in correct format")]
    InvalidConfig(#[from] toml::de::Error),
    #[error("invalid help state")]
    InvalidHelpState(#[from] InvalidHelpState),
    #[error("io error")]
    Io(#[from] tokio::io::Error),
    #[error("error while downloading map")]
    MapDownload(#[from] map_download::MapDownloadError),
    #[error("interaction contained neighter member nor user")]
    MissingInteractionAuthor,
    #[error("config file was not found")]
    NoConfig,
    #[error("osu error")]
    Osu(#[from] rosu_v2::error::OsuError),
    #[error("error while calculating pp")]
    Pp(#[from] pp::PPError),
    #[error("error while communicating with redis cache")]
    Redis(#[from] deadpool_redis::redis::RedisError),
    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),
    #[error("serde json error")]
    Json(#[from] serde_json::Error),
    #[error("shard command error")]
    ShardCommand(#[from] twilight_gateway::shard::CommandError),
    #[error("twilight failed to deserialize response")]
    TwilightDeserialize(#[from] twilight_http::response::DeserializeBodyError),
    #[error("error while making discord request")]
    TwilightHttp(#[from] twilight_http::Error),
    #[error("twitch error")]
    Twitch(#[from] twitch::TwitchError),
    #[error("unknown message component: {component:#?}")]
    UnknownMessageComponent {
        component: Box<MessageComponentInteraction>,
    },
    #[error("unknown slash command `{name}`: {command:#?}")]
    UnknownSlashCommand {
        name: String,
        command: Box<ApplicationCommand>,
    },
    #[error("error while updating message")]
    UpdateMessage(#[from] UpdateMessageError),
    #[error("update original response error")]
    UpdateOriginalResponse(#[from] UpdateOriginalResponseError),
}

#[derive(Debug, thiserror::Error)]
#[error("failed to create graph")]
pub enum GraphError {
    Image(#[from] image::ImageError),
    #[error("failed to create graph: no non-zero strain point")]
    InvalidStrainPoints,
    #[error("failed to create graph: {0}")]
    Plotter(String),
    Reqwest(#[from] reqwest::Error),
}

impl<E: std::error::Error + Send + Sync> From<DrawingAreaErrorKind<E>> for GraphError {
    fn from(err: DrawingAreaErrorKind<E>) -> Self {
        Self::Plotter(err.to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InvalidHelpState {
    #[error("unknown command")]
    UnknownCommand,
    #[error("missing embed")]
    MissingEmbed,
    #[error("missing embed title")]
    MissingTitle,
}
