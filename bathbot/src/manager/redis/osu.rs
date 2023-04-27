use std::borrow::Cow;

use bathbot_cache::Cache;
use bathbot_model::rosu_v2::user::{StatsWrapper, User};
use bathbot_util::{
    constants::OSU_BASE, numbers::WithComma, osu::flag_url, AuthorBuilder, CowUtils,
};
use rosu_v2::{
    prelude::{GameMode, OsuError},
    request::UserId,
};

use super::{RedisData, RedisManager, RedisResult};
use crate::core::Context;

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

    pub async fn rosu_id(ctx: &Context, user_id: &UserId) -> Self {
        match user_id {
            UserId::Id(user_id) => Self::user_id(*user_id),
            UserId::Name(name) => Self::username(ctx, name).await,
        }
    }

    // TODO: require mode already
    pub async fn username(ctx: &Context, name: impl AsRef<str>) -> Self {
        let name = name.as_ref();
        let alt_name = Self::alt_name(name);

        match ctx.osu_user().user_id(name, alt_name.as_deref()).await {
            Ok(Some(user_id)) => return Self::Args(UserArgsSlim::user_id(user_id)),
            Err(err) => warn!(?err, "Failed to get user id"),
            Ok(None) => {}
        }

        let mode = GameMode::Osu;

        match (ctx.osu().user(name).mode(mode).await, alt_name) {
            (Ok(user), _) => {
                if let Err(err) = ctx.osu_user().store_user(&user, mode).await {
                    warn!("{err:?}");
                }

                Self::User {
                    user: Box::new(user.into()),
                    mode: GameMode::Osu,
                }
            }
            (Err(OsuError::NotFound), Some(alt_name)) => {
                match ctx.osu().user(alt_name).mode(mode).await {
                    Ok(user) => {
                        if let Err(err) = ctx.osu_user().store_user(&user, mode).await {
                            warn!("{err:?}");
                        }

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

const EXPIRE: usize = 600;

impl<'c> RedisManager<'c> {
    fn osu_user_key(user_id: u32, mode: GameMode) -> String {
        format!("osu_user_{user_id}_{}", mode as u8)
    }

    pub async fn osu_user_from_args(self, args: UserArgsSlim) -> RedisResult<User, User, OsuError> {
        let UserArgsSlim { user_id, mode } = args;
        let key = Self::osu_user_key(user_id, mode);

        let mut conn = match self.ctx.cache.fetch(&key).await {
            Ok(Ok(user)) => {
                self.ctx.stats.inc_cached_user();

                return Ok(RedisData::Archive(user));
            }
            Ok(Err(conn)) => Some(conn),
            Err(err) => {
                warn!("{err:?}");

                None
            }
        };

        let mut user = match self.ctx.osu().user(user_id).mode(mode).await {
            Ok(user) => user,
            Err(OsuError::NotFound) => {
                // Remove stats of unknown/restricted users so they don't appear in the
                // leaderboard
                if let Err(err) = self.ctx.osu_user().remove_stats_and_scores(user_id).await {
                    warn!(?err, "Failed to remove stats of unknown user");
                }

                return Err(OsuError::NotFound);
            }
            Err(err) => return Err(err),
        };

        user.mode = mode;

        if let Err(err) = self.ctx.osu_user().store_user(&user, mode).await {
            warn!("{err:?}");
        }

        let user = User::from(user);

        if let Some(ref mut conn) = conn {
            // Cache users for 10 minutes
            if let Err(err) = Cache::store::<_, _, 64>(conn, &key, &user, EXPIRE).await {
                warn!(?err, "Failed to store user");
            }
        }

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
        let store_fut = self.ctx.cache.store_new::<_, _, 64>(&key, &user, EXPIRE);

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
}
