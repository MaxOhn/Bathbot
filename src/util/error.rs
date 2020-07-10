use darkredis::Error as RedisError;
use deadpool_postgres::PoolError;
use refinery::Error as MigrationError;
use serde_json::Error as SerdeJsonError;
use std::{error, fmt};
use tokio_postgres::Error as DBError;
use toml::de::Error as TomlError;
use twilight::{gateway::cluster::Error as ClusterError, http::Error as HttpError};

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
    Custom(String),
    Database(DBError),
    Fmt(fmt::Error),
    InvalidConfig(TomlError),
    InvalidSession(u64),
    Migration(MigrationError),
    NoConfig,
    NoLoggingSpec,
    Pool(PoolError),
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
                write!(f, "Error defrosting cache ({}): {}", reason, e)
            }
            Error::Custom(e) => write!(f, "{}", e),
            Error::Database(e) => write!(f, "Database error occured: {}", e),
            Error::Fmt(e) => write!(f, "fmt error: {}", e),
            Error::InvalidConfig(e) => {
                write!(f, "The config file was not in the correct format: {}", e)
            }
            Error::InvalidSession(shard) => write!(
                f,
                "Gateway invalidated session unrecoverably for shard {}",
                shard
            ),
            Error::Migration(e) => write!(f, "Error while migrating database schema: {}", e),
            Error::NoConfig => write!(f, "The config file could not be found"),
            Error::NoLoggingSpec => write!(f, "The logging configuration could not be found"),
            Error::Pool(e) => write!(f, "Error with postgres pool: {}", e),
            Error::Redis(e) => write!(f, "Error communicating with redis cache: {}", e),
            Error::Serde(e) => write!(f, "Serde error: {}", e),
            Error::TwilightHttp(e) => write!(f, "Error while making a Discord request: {}", e),
            Error::TwilightCluster(e) => write!(f, "Error occurred on a cluster request: {}", e),
        }
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

impl From<MigrationError> for Error {
    fn from(e: MigrationError) -> Self {
        Error::Migration(e)
    }
}

impl From<PoolError> for Error {
    fn from(e: PoolError) -> Self {
        Error::Pool(e)
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
