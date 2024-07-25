use std::borrow::Cow;

use bathbot_cache::Cache;
use bathbot_model::rosu_v2::user::{ArchivedUserHighestRank, StatsWrapper, User};
use bathbot_util::{
    constants::OSU_BASE, numbers::WithComma, osu::flag_url, AuthorBuilder, CowUtils,
};
use rosu_v2::{
    prelude::{GameMode, OsuError, User as RosuUser},
    request::UserId,
};

use super::{RedisData, RedisManager, RedisResult};
use crate::core::{BotMetrics, Context};

/// Retrieve an osu user through redis or the osu!api as backup
pub enum UserArgs {
    Args(UserArgsSlim),
    User { user: Box<User>, mode: GameMode },
    Err(OsuError),
}

impl UserArgs {
    pub fn user_id(user_id: u32) -> Self {
        Self::Args(UserArgsSlim::user_id(user_id))
    }

    pub async fn rosu_id(user_id: &UserId) -> Self {
        match user_id {
            UserId::Id(user_id) => Self::user_id(*user_id),
            UserId::Name(name) => Self::username(name).await,
        }
    }

    // TODO: require mode already
    pub async fn username(name: impl AsRef<str>) -> Self {
        let name = name.as_ref();
        let alt_name = Self::alt_name(name);

        match Context::osu_user().user_id(name, alt_name.as_deref()).await {
            Ok(Some(user_id)) => return Self::Args(UserArgsSlim::user_id(user_id)),
            Err(err) => warn!(?err, "Failed to get user id"),
            Ok(None) => {}
        }

        let mode = GameMode::Osu;

        match (Context::osu().user(name).mode(mode).await, alt_name) {
            (Ok(user), _) => {
                let user_clone = user.clone();

                tokio::spawn(async move {
                    Context::osu_user().store(&user_clone, mode).await;
                    Context::get()
                        .notify_osutrack_of_user_activity(user_clone.user_id, mode)
                        .await;
                });

                Self::User {
                    user: Box::new(user.into()),
                    mode: GameMode::Osu,
                }
            }
            (Err(OsuError::NotFound), Some(alt_name)) => {
                match Context::osu().user(alt_name).mode(mode).await {
                    Ok(user) => {
                        let user_clone = user.clone();

                        tokio::spawn(async move {
                            Context::osu_user().store(&user_clone, mode).await;
                            Context::get()
                                .notify_osutrack_of_user_activity(user_clone.user_id, mode)
                                .await;
                        });

                        Self::User {
                            user: Box::new(user.into()),
                            mode,
                        }
                    }
                    Err(err) => Self::Err(err),
                }
            }
            (Err(err), _) => Self::Err(err),
        }
    }

    pub fn mode(self, mode: GameMode) -> Self {
        match self {
            Self::Args(args) => Self::Args(args.mode(mode)),
            Self::User { user, mode: mode_ } => {
                if mode == mode_ {
                    Self::User { user, mode }
                } else {
                    Self::user_id(user.user_id).mode(mode)
                }
            }
            Self::Err(err) => Self::Err(err),
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
    type Error = OsuError;

    #[inline]
    fn try_from(args: UserArgs) -> Result<Self, Self::Error> {
        match args {
            UserArgs::Args(args) => Ok(args),
            UserArgs::User { user, mode } => Ok(Self::user_id(user.user_id).mode(mode)),
            UserArgs::Err(err) => Err(err),
        }
    }
}

const EXPIRE: u64 = 600;

impl RedisManager {
    fn osu_user_key(user_id: u32, mode: GameMode) -> String {
        format!("osu_user_{user_id}_{}", mode as u8)
    }

    pub async fn osu_user_from_args(self, args: UserArgsSlim) -> RedisResult<User, User, OsuError> {
        let UserArgsSlim { user_id, mode } = args;
        let key = Self::osu_user_key(user_id, mode);

        let mut conn = match Context::cache().fetch(&key).await {
            Ok(Ok(user)) => {
                BotMetrics::inc_redis_hit("osu! user");

                return Ok(RedisData::Archive(user));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let mut user = match Context::osu().user(user_id).mode(mode).await {
            Ok(user) => user,
            Err(OsuError::NotFound) => {
                // Remove stats of unknown/restricted users so they don't appear in the
                // leaderboard
                if let Err(err) = Context::osu_user().remove_stats_and_scores(user_id).await {
                    warn!(?err, "Failed to remove stats of unknown user");
                }

                return Err(OsuError::NotFound);
            }
            Err(err) => return Err(err),
        };

        user.mode = mode;
        let user_clone = user.clone();
        let user = User::from(user);

        if let Some(ref mut conn) = conn {
            // Cache users for 10 minutes
            if let Err(err) = Cache::store::<_, _, 64>(conn, &key, &user, EXPIRE).await {
                warn!(?err, "Failed to store user");
            }
        }

        drop(conn);

        tokio::spawn(async move {
            Context::osu_user().store(&user_clone, mode).await;
            Context::get()
                .notify_osutrack_of_user_activity(user_clone.user_id, mode)
                .await;
        });

        Ok(RedisData::new(user))
    }

    pub async fn osu_user_from_user(
        self,
        mut user: User,
        mode: GameMode,
    ) -> RedisResult<User, User, OsuError> {
        let key = Self::osu_user_key(user.user_id, mode);

        user.mode = mode;

        // Cache users for 10 minutes
        let store_fut = Context::cache().store_new::<_, _, 64>(&key, &user, EXPIRE);

        if let Err(err) = store_fut.await {
            warn!(?err, "Failed to store user");
        }

        Ok(RedisData::Original(user))
    }

    pub async fn osu_user(self, args: UserArgs) -> RedisResult<User, User, OsuError> {
        match args {
            UserArgs::Args(args) => self.osu_user_from_args(args).await,
            UserArgs::User { user, mode } => self.osu_user_from_user(*user, mode).await,
            UserArgs::Err(err) => Err(err),
        }
    }
}

impl RedisData<User> {
    pub fn avatar_url(&self) -> &str {
        match self {
            RedisData::Original(user) => user.avatar_url.as_ref(),
            RedisData::Archive(user) => user.avatar_url.as_ref(),
        }
    }

    pub fn user_id(&self) -> u32 {
        match self {
            RedisData::Original(user) => user.user_id,
            RedisData::Archive(user) => user.user_id,
        }
    }

    pub fn username(&self) -> &str {
        match self {
            RedisData::Original(user) => user.username.as_str(),
            RedisData::Archive(user) => user.username.as_str(),
        }
    }

    pub fn mode(&self) -> GameMode {
        match self {
            RedisData::Original(user) => user.mode,
            RedisData::Archive(user) => user.mode,
        }
    }

    pub fn country_code(&self) -> &str {
        match self {
            RedisData::Original(user) => user.country_code.as_str(),
            RedisData::Archive(user) => user.country_code.as_str(),
        }
    }

    pub fn stats(&self) -> StatsWrapper<'_> {
        let stats_opt = match self {
            RedisData::Original(user) => user.statistics.as_ref().map(StatsWrapper::Left),
            RedisData::Archive(user) => user.statistics.as_ref().map(StatsWrapper::Right),
        };

        stats_opt.expect("missing statistics")
    }

    pub fn author_builder(&self) -> AuthorBuilder {
        match self {
            RedisData::Original(user) => {
                let stats = user.statistics.as_ref().expect("missing statistics");

                let text = format!(
                    "{name}: {pp}pp (#{global} {country}{national})",
                    name = user.username,
                    pp = WithComma::new(stats.pp),
                    global = WithComma::new(stats.global_rank.unwrap_or(0)),
                    country = user.country_code,
                    national = stats.country_rank.unwrap_or(0)
                );

                let url = format!("{OSU_BASE}users/{}/{}", user.user_id, user.mode);
                let icon = flag_url(&user.country_code);

                AuthorBuilder::new(text).url(url).icon_url(icon)
            }
            RedisData::Archive(user) => {
                let stats = user.statistics.as_ref().expect("missing statistics");
                let country_code = user.country_code.as_str();

                let text = format!(
                    "{name}: {pp}pp (#{global} {country_code}{national})",
                    name = user.username,
                    pp = WithComma::new(stats.pp),
                    global = WithComma::new(stats.global_rank),
                    national = stats.country_rank
                );

                let url = format!("{OSU_BASE}users/{}/{}", user.user_id, user.mode);
                let icon = flag_url(country_code);

                AuthorBuilder::new(text).url(url).icon_url(icon)
            }
        }
    }

    pub fn update(&mut self, user: RosuUser) {
        match self {
            RedisData::Original(user_) => {
                let RosuUser {
                    avatar_url,
                    country_code,
                    last_visit,
                    user_id,
                    username,
                    badges,
                    follower_count,
                    graveyard_mapset_count,
                    guest_mapset_count,
                    highest_rank,
                    loved_mapset_count,
                    monthly_playcounts,
                    rank_history,
                    ranked_mapset_count,
                    replays_watched_counts,
                    scores_first_count,
                    statistics,
                    pending_mapset_count,
                    medals,
                    ..
                } = user;

                user_.avatar_url = avatar_url.into_boxed_str();
                user_.country_code = country_code;
                user_.user_id = user_id;
                user_.username = username;

                if let last_visit @ Some(_) = last_visit {
                    user_.last_visit = last_visit;
                }

                if let Some(badges) = badges {
                    user_.badges = badges;
                }

                if let Some(follower_count) = follower_count {
                    user_.follower_count = follower_count;
                }

                if let Some(graveyard_mapset_count) = graveyard_mapset_count {
                    user_.graveyard_mapset_count = graveyard_mapset_count
                }

                if let Some(guest_mapset_count) = guest_mapset_count {
                    user_.guest_mapset_count = guest_mapset_count
                }

                if let highest_rank @ Some(_) = highest_rank {
                    user_.highest_rank = highest_rank;
                }

                if let Some(loved_mapset_count) = loved_mapset_count {
                    user_.loved_mapset_count = loved_mapset_count;
                }

                if let Some(monthly_playcounts) = monthly_playcounts {
                    user_.monthly_playcounts = monthly_playcounts
                }

                if let Some(rank_history) = rank_history {
                    user_.rank_history = rank_history.into_boxed_slice();
                }

                if let Some(ranked_mapset_count) = ranked_mapset_count {
                    user_.ranked_mapset_count = ranked_mapset_count;
                }

                if let Some(replays_watched_counts) = replays_watched_counts {
                    user_.replays_watched_counts = replays_watched_counts;
                }

                if let Some(scores_first_count) = scores_first_count {
                    user_.scores_first_count = scores_first_count;
                }

                if let Some(pending_mapset_count) = pending_mapset_count {
                    user_.pending_mapset_count = pending_mapset_count;
                }

                if let Some(medals) = medals {
                    user_.medals = medals;
                }

                if let statistics @ Some(_) = statistics {
                    user_.statistics = statistics;
                }
            }
            RedisData::Archive(user_) => user_.mutate(|mut archived| {
                if let Some(last_visit) = user.last_visit {
                    // SAFETY: The modified option will keep its variant and
                    // i128 is Unpin.
                    let last_visit_ = unsafe {
                        archived
                            .as_mut()
                            .map_unchecked_mut(|user| &mut user.last_visit)
                    };

                    if let Some(last_visit_) = last_visit_.as_pin_mut() {
                        *last_visit_.get_mut() = last_visit.unix_timestamp_nanos();
                    }
                }

                if let Some(stats) = user.statistics {
                    // SAFETY: The modified option will keep its variant and
                    // UserStatistics is Unpin.
                    let stats_ = unsafe {
                        archived
                            .as_mut()
                            .map_unchecked_mut(|user| &mut user.statistics)
                    };

                    if let Some(stats_) = stats_.as_pin_mut() {
                        *stats_.get_mut() = stats.into();
                    }
                }

                if let Some(highest_rank) = user.highest_rank {
                    // SAFETY: The modified option will keep its variant and
                    // ArchivedUserHighestRank is Unpin.
                    let highest_rank_ = unsafe {
                        archived
                            .as_mut()
                            .map_unchecked_mut(|user| &mut user.highest_rank)
                    };

                    if let Some(highest_rank_) = highest_rank_.as_pin_mut() {
                        *highest_rank_.get_mut() = ArchivedUserHighestRank {
                            rank: highest_rank.rank,
                            updated_at: highest_rank.updated_at.unix_timestamp_nanos(),
                        };
                    }
                }

                macro_rules! update_pod {
                    ( $field:ident ) => {
                        if let Some($field) = user.$field {
                            // SAFETY: The modified option will keep its variant and
                            // POD is Unpin.
                            let field = unsafe {
                                archived.as_mut().map_unchecked_mut(|user| &mut user.$field)
                            };

                            *field.get_mut() = $field;
                        }
                    };
                }

                update_pod!(follower_count);
                update_pod!(graveyard_mapset_count);
                update_pod!(guest_mapset_count);
                update_pod!(loved_mapset_count);
                update_pod!(ranked_mapset_count);
                update_pod!(scores_first_count);
                update_pod!(pending_mapset_count);
            }),
        }
    }
}
