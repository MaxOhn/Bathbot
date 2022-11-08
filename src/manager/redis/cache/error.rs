use std::convert::Infallible;

use bb8_redis::{bb8::RunError, redis::RedisError};
use rkyv::ser::serializers::{
    AllocScratchError, CompositeSerializerError, SharedSerializeMapError,
};
use thiserror::Error;

#[derive(Debug)]
pub(super) enum ColdResumeErrorKind {
    Channels,
    CurrentUser,
    Guilds,
    Members,
    ResumeData,
    Roles,
    Users,
}

#[derive(Debug, Error)]
#[error("{:?} could not be defrosted", .kind)]
pub struct DefrostError {
    pub(super) kind: ColdResumeErrorKind,
    #[source]
    pub(super) inner: DefrostInnerError,
}

#[derive(Debug, Error)]
pub(super) enum DefrostInnerError {
    #[error("missing redis key `{0}`")]
    MissingKey(String),
    #[error("redis pool error")]
    Pool(#[from] RunError<RedisError>),
    #[error("redis error")]
    Redis(#[from] RedisError),
}

#[derive(Debug, Error)]
#[error("{:?} could not be frozen", .kind)]
pub struct FreezeError {
    pub(super) kind: ColdResumeErrorKind,
    #[source]
    pub(super) inner: FreezeInnerError,
}

type AllocSerializerError =
    CompositeSerializerError<Infallible, AllocScratchError, SharedSerializeMapError>;

#[derive(Debug, Error)]
pub(super) enum FreezeInnerError {
    #[error("missing current user in cache")]
    MissingCurrentUser,
    #[error("redis pool error")]
    Pool(#[from] RunError<RedisError>),
    #[error("redis error")]
    Redis(#[from] RedisError),
    #[error("serializer error")]
    Serializer(#[from] AllocSerializerError),
}
