use crate::roppai::OppaiErr;

use darkredis::Error as RedisError;
use serde_json::Error as SerdeJsonError;
use sqlx::Error as DBError;
use std::{borrow::Cow, error, fmt};
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
    UpdateMessage(UpdateMessageError),
    Custom(String),
    Database(DBError),
    Fmt(fmt::Error),
    InvalidConfig(TomlError),
    InvalidSession(u64),
    NoConfig,
    NoLoggingSpec,
    Oppai(OppaiErr),
    Redis(RedisError),
    Serde(SerdeJsonError),
    TwilightHttp(HttpError),
    TwilightCluster(ClusterError),
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::CacheDefrost(reason, e) => {
                write!(f, "error defrosting cache ({}): {}", reason, e)
            }
            Error::Command(cmd, e) => write!(f, "error while processing command `{}`: {}", cmd, e),
            Error::CreateMessage(e) => {
                f.write_str("error while creating message: ")?;
                if let CreateMessageError::EmbedTooLarge { source } = e {
                    source.fmt(f)
                } else {
                    e.fmt(f)
                }
            }
            Error::UpdateMessage(e) => {
                f.write_str("error while updating message: ")?;
                if let UpdateMessageError::EmbedTooLarge { source } = e {
                    source.fmt(f)
                } else {
                    e.fmt(f)
                }
            }
            Error::Custom(e) => e.fmt(f),
            Error::Database(e) => write!(f, "database error occured: {}", e),
            Error::Fmt(e) => write!(f, "fmt error: {}", e),
            Error::InvalidConfig(e) => write!(f, "config file was not in correct format: {}", e),
            Error::InvalidSession(shard) => write!(
                f,
                "gateway invalidated session unrecoverably for shard {}",
                shard
            ),
            Error::NoConfig => f.write_str("config file was not found"),
            Error::NoLoggingSpec => f.write_str("logging config was not found"),
            Error::Oppai(e) => write!(f, "error while using oppai: {}", e),
            Error::Redis(e) => write!(f, "error while communicating with redis cache: {}", e),
            Error::Serde(e) => write!(f, "serde error: {}", e),
            Error::TwilightHttp(e) => write!(f, "error while making discord request: {}", e),
            Error::TwilightCluster(e) => write!(f, "error occurred on cluster request: {}", e),
        }
    }
}

impl From<CreateMessageError> for Error {
    fn from(e: CreateMessageError) -> Self {
        Error::CreateMessage(e)
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
