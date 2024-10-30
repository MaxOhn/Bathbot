use std::borrow::Cow;

use bathbot_cache::{data::BathbotRedisData, osu::user::CacheOsuUser, value::CachedArchive};
use bathbot_model::rosu_v2::user::ArchivedUser;
use bathbot_util::CowUtils;
use eyre::{Report, WrapErr};
use rosu_v2::{
    prelude::{GameMode, OsuError, UserExtended},
    request::UserId,
};

use super::RedisManager;
use crate::core::{BotMetrics, Context};

pub type CachedOsuUser = CachedArchive<ArchivedUser>;

/// Retrieve an osu user through redis or the osu!api as backup
pub enum UserArgs {
    Args(UserArgsSlim),
    User { user: CachedOsuUser, mode: GameMode },
    Err(UserArgsError),
}

#[derive(Debug, thiserror::Error)]
pub enum UserArgsError {
    #[error("osu error")]
    Osu(#[from] OsuError),
    #[error("serialization error")]
    Serialization(#[source] Report),
    #[error("validation error")]
    Validation(#[source] Report),
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

        let archived = match CacheOsuUser::serialize(&user) {
            Ok(bytes) => match CachedArchive::new(bytes) {
                Ok(archived) => archived,
                Err(err) => return Self::Err(UserArgsError::Validation(err)),
            },
            Err(err) => return Self::Err(UserArgsError::Serialization(Report::new(err))),
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

impl RedisManager {
    fn osu_user_key(user_id: u32, mode: GameMode) -> String {
        format!("osu_user_{user_id}_{}", mode as u8)
    }

    pub async fn osu_user_from_args(
        self,
        args: UserArgsSlim,
    ) -> Result<CachedOsuUser, UserArgsError> {
        let UserArgsSlim { user_id, mode } = args;
        let key = Self::osu_user_key(user_id, mode);

        match Context::cache().fetch::<CacheOsuUser>(&key).await {
            Ok(Some(data)) => {
                BotMetrics::inc_redis_hit("osu! user");

                return Ok(data);
            }
            Ok(None) => {}
            Err(err) => warn!("{err:?}"),
        }

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

        let bytes = CacheOsuUser::serialize(&user)
            .wrap_err_with(|| format!("Failed to serialize {key}"))
            .map_err(UserArgsError::Serialization)?;

        let store_fut = Context::cache().store_serialized::<CacheOsuUser>(&key, bytes.as_slice());

        if let Err(err) = store_fut.await {
            warn!(?err, "Failed to store {key}");
        }

        tokio::spawn(async move {
            Context::osu_user().store(&user, mode).await;
            Context::get()
                .notify_osutrack_of_user_activity(user.user_id, mode)
                .await;
        });

        CachedArchive::new(bytes).map_err(UserArgsError::Validation)
    }

    pub async fn osu_user_from_archived(
        self,
        user: CachedOsuUser,
        mode: GameMode,
    ) -> CachedOsuUser {
        let key = Self::osu_user_key(user.user_id.to_native(), mode);
        let bytes = user.bytes();
        let store_fut = Context::cache().store_serialized::<CacheOsuUser>(&key, bytes);

        if let Err(err) = store_fut.await {
            warn!(?err, "Failed to store {key}");
        }

        user
    }

    pub async fn osu_user(self, args: UserArgs) -> Result<CachedOsuUser, UserArgsError> {
        match args {
            UserArgs::Args(args) => self.osu_user_from_args(args).await,
            UserArgs::User { user, mode } => Ok(self.osu_user_from_archived(user, mode).await),
            UserArgs::Err(err) => Err(err),
        }
    }
}

// TODO
// impl ArchivedUser {
//     pub fn update_(&mut self, user: RosuUser) {
//         self.mutate(|mut archived| {
//             if let Some(last_visit) = user.last_visit {
//                 // SAFETY: The modified option will keep its variant and
//                 // i128 is Unpin.
//                 let last_visit_ = unsafe {
//                     archived
//                         .as_mut()
//                         .map_unchecked_mut(|user| &mut user.last_visit)
//                 };

//                 if let Some(last_visit_) = last_visit_.as_pin_mut() {
//                     *last_visit_.get_mut() =
// last_visit.unix_timestamp_nanos();                 }
//             }

//             if let Some(stats) = user.statistics {
//                 // SAFETY: The modified option will keep its variant and
//                 // UserStatistics is Unpin.
//                 let stats_ = unsafe {
//                     archived
//                         .as_mut()
//                         .map_unchecked_mut(|user| &mut user.statistics)
//                 };

//                 if let Some(stats_) = stats_.as_pin_mut() {
//                     *stats_.get_mut() = stats.into();
//                 }
//             }

//             if let Some(highest_rank) = user.highest_rank {
//                 // SAFETY: The modified option will keep its variant and
//                 // ArchivedUserHighestRank is Unpin.
//                 let highest_rank_ = unsafe {
//                     archived
//                         .as_mut()
//                         .map_unchecked_mut(|user| &mut user.highest_rank)
//                 };

//                 if let Some(highest_rank_) = highest_rank_.as_pin_mut() {
//                     *highest_rank_.get_mut() = ArchivedUserHighestRank {
//                         rank: highest_rank.rank,
//                         updated_at:
// highest_rank.updated_at.unix_timestamp_nanos(),                     };
//                 }
//             }

//             macro_rules! update_pod {
//                 ( $field:ident ) => {
//                     if let Some($field) = user.$field {
//                         // SAFETY: The modified option will keep its variant
// and                         // POD is Unpin.
//                         let field =
//                             unsafe {
// archived.as_mut().map_unchecked_mut(|user| &mut user.$field) };

//                         *field.get_mut() = $field;
//                     }
//                 };
//             }

//             update_pod!(follower_count);
//             update_pod!(graveyard_mapset_count);
//             update_pod!(guest_mapset_count);
//             update_pod!(loved_mapset_count);
//             update_pod!(ranked_mapset_count);
//             update_pod!(scores_first_count);
//             update_pod!(pending_mapset_count);
//         })
//     }
// }
