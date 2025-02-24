use std::borrow::Cow;

use bathbot_cache::{Cache, model::CachedArchive, util::serialize::serialize_using_arena_and_with};
use bathbot_model::rosu_v2::user::{ArchivedUser, User};
use bathbot_util::CowUtils;
use rkyv::rancor::BoxedError;
use rosu_v2::{
    prelude::{GameMode, OsuError, UserExtended},
    request::UserId,
};

use super::RedisManager;
use crate::core::{BotMetrics, Context};

pub type CachedUser = CachedArchive<ArchivedUser>;

/// Retrieve an osu user through redis or the osu!api as backup
pub enum UserArgs {
    Args(UserArgsSlim),
    User { user: CachedUser, mode: GameMode },
    Err(UserArgsError),
}

#[derive(Debug, thiserror::Error)]
pub enum UserArgsError {
    #[error("osu! error")]
    Osu(#[from] OsuError),
    #[error("Failed to serialize data; {user:?}")]
    Serialization {
        #[source]
        source: BoxedError,
        user: Box<UserExtended>,
    },
    #[error("Failed to validate data")]
    Validation(#[source] BoxedError),
}

impl UserArgs {
    pub fn user_id(user_id: u32, mode: GameMode) -> Self {
        Self::Args(UserArgsSlim::user_id(user_id).mode(mode))
    }

    pub async fn rosu_id(user_id: &UserId, mode: GameMode) -> Self {
        match user_id {
            UserId::Id(user_id) => Self::user_id(*user_id, mode),
            UserId::Name(name) => Self::username(name, mode).await,
        }
    }

    pub async fn username(name: impl AsRef<str>, mode: GameMode) -> Self {
        let name = name.as_ref();
        let alt_name = Self::alt_name(name);

        match Context::osu_user().user_id(name, alt_name.as_deref()).await {
            Ok(Some(user_id)) => return Self::user_id(user_id, mode),
            Err(err) => warn!(?err, "Failed to get user id"),
            Ok(None) => {}
        }

        match (Context::osu().user(name).mode(mode).await, alt_name) {
            (Ok(user), _) => Self::from_user(user, mode),
            (Err(OsuError::NotFound), Some(alt_name)) => {
                match Context::osu().user(alt_name).mode(mode).await {
                    Ok(user) => Self::from_user(user, mode),
                    Err(err) => Self::Err(UserArgsError::Osu(err)),
                }
            }
            (Err(err), _) => Self::Err(UserArgsError::Osu(err)),
        }
    }

    pub fn alt_name(name: &str) -> Option<String> {
        if name.starts_with('_') || name.ends_with('_') {
            None
        } else if let Cow::Owned(name) = name.cow_replace('_', " ") {
            Some(name)
        } else {
            None
        }
    }

    fn from_user(mut user: UserExtended, mode: GameMode) -> Self {
        user.mode = mode;

        let archived = match serialize_using_arena_and_with::<_, User>(&user) {
            Ok(bytes) => match CachedUser::new(bytes) {
                Ok(archived) => archived,
                Err(err) => return Self::Err(UserArgsError::Validation(err)),
            },
            Err(source) => {
                return Self::Err(UserArgsError::Serialization {
                    source,
                    user: Box::new(user),
                });
            }
        };

        tokio::spawn(async move {
            Context::osu_user().store(&user, mode).await;
            Context::get()
                .notify_osutrack_of_user_activity(user.user_id, mode)
                .await;
        });

        Self::User {
            user: archived,
            mode,
        }
    }
}

/// Retrieve an osu user through redis or the osu!api as backup
#[derive(Copy, Clone)]
pub struct UserArgsSlim {
    pub user_id: u32,
    pub mode: GameMode,
}

impl UserArgsSlim {
    pub fn user_id(user_id: u32) -> Self {
        Self {
            user_id,
            mode: GameMode::Osu,
        }
    }

    pub fn mode(mut self, mode: GameMode) -> Self {
        self.mode = mode;

        self
    }
}

impl TryFrom<UserArgs> for UserArgsSlim {
    type Error = UserArgsError;

    #[inline]
    fn try_from(args: UserArgs) -> Result<Self, Self::Error> {
        match args {
            UserArgs::Args(args) => Ok(args),
            UserArgs::User { user, mode } => Ok(Self::user_id(user.user_id.to_native()).mode(mode)),
            UserArgs::Err(err) => Err(err),
        }
    }
}

const EXPIRE: u64 = 600;

impl RedisManager {
    fn osu_user_key(user_id: u32, mode: GameMode) -> String {
        format!("osu_user_{user_id}_{}", mode as u8)
    }

    pub async fn osu_user_from_args(self, args: UserArgsSlim) -> Result<CachedUser, UserArgsError> {
        let UserArgsSlim { user_id, mode } = args;
        let key = Self::osu_user_key(user_id, mode);

        let mut conn = match Context::cache().fetch(&key).await {
            Ok(Ok(user)) => {
                BotMetrics::inc_redis_hit("osu! user");

                return Ok(user);
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!(?err, "Failed to fetch osu! user");

                None
            }
        };

        let mut user = match Context::osu().user(user_id).mode(mode).await {
            Ok(user) => user,
            Err(err @ OsuError::NotFound) => {
                // Remove stats of unknown/restricted users so they don't appear in the
                // leaderboard
                if let Err(err) = Context::osu_user().remove_stats_and_scores(user_id).await {
                    warn!(?err, "Failed to remove stats of unknown user");
                }

                return Err(UserArgsError::Osu(err));
            }
            Err(err) => return Err(UserArgsError::Osu(err)),
        };

        user.mode = mode;

        let bytes = match serialize_using_arena_and_with::<_, User>(&user) {
            Ok(bytes) => bytes,
            Err(source) => {
                return Err(UserArgsError::Serialization {
                    source,
                    user: Box::new(user),
                });
            }
        };

        if let Some(ref mut conn) = conn {
            // Cache users for 10 minutes
            if let Err(err) = Cache::store(conn, &key, bytes.as_slice(), EXPIRE).await {
                warn!(?err, "Failed to store user");
            }
        }

        tokio::spawn(async move {
            Context::osu_user().store(&user, mode).await;
            Context::get()
                .notify_osutrack_of_user_activity(user.user_id, mode)
                .await;
        });

        CachedUser::new(bytes).map_err(UserArgsError::Validation)
    }

    pub async fn osu_user_from_archived(self, user: CachedUser, mode: GameMode) -> CachedUser {
        let key = Self::osu_user_key(user.user_id.to_native(), mode);
        let bytes = user.as_bytes();
        let store_fut = Context::cache().store_new(&key, bytes, EXPIRE);

        if let Err(err) = store_fut.await {
            warn!(?err, "Failed to store key {key}");
        }

        user
    }

    pub async fn osu_user(self, args: UserArgs) -> Result<CachedUser, UserArgsError> {
        match args {
            UserArgs::Args(args) => self.osu_user_from_args(args).await,
            UserArgs::User { user, mode } => Ok(self.osu_user_from_archived(user, mode).await),
            UserArgs::Err(err) => Err(err),
        }
    }
}
