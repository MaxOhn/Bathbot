use std::borrow::Cow;

use bathbot_cache::Cache;
use bathbot_model::rkyv_impls::{CountryCodeWrapper, DateTimeWrapper, UsernameWrapper};
use bathbot_util::{
    constants::OSU_BASE, numbers::WithComma, osu::flag_url, AuthorBuilder, CowUtils,
};
use rkyv::{
    with::{DeserializeWith, Map},
    Archive, Deserialize, Infallible, Serialize,
};
use rosu_v2::{
    prelude::{
        Badge, CountryCode, GameMode, MedalCompact, MonthlyCount, OsuError, User as RosuUser,
        UserHighestRank, UserKudosu, UserStatistics, Username,
    },
    request::UserId,
};
use time::OffsetDateTime;

use crate::core::Context;

use super::{RedisData, RedisManager, RedisResult};

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
            Err(err) => warn!("{:?}", err.wrap_err("Failed to get user id")),
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

    pub async fn osu_user_from_args(self, args: UserArgsSlim) -> RedisResult<User, OsuError> {
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
                // Remove stats of unknown/restricted users so they don't appear in the leaderboard
                if let Err(err) = self.ctx.osu_user().remove_stats(user_id).await {
                    let wrap = "failed to remove stats of unknown user";
                    warn!("{:?}", err.wrap_err(wrap));
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
                warn!("{:?}", err.wrap_err("Failed to store user"));
            }
        }

        Ok(RedisData::new(user))
    }

    pub async fn osu_user_from_user(
        self,
        mut user: User,
        mode: GameMode,
    ) -> RedisResult<User, OsuError> {
        let key = Self::osu_user_key(user.user_id, mode);

        user.mode = mode;

        // Cache users for 10 minutes
        let store_fut = self.ctx.cache.store_new::<_, _, 64>(&key, &user, EXPIRE);

        if let Err(err) = store_fut.await {
            warn!("{:?}", err.wrap_err("Failed to store user"));
        }

        Ok(RedisData::Original(user))
    }

    pub async fn osu_user(self, args: UserArgs) -> RedisResult<User, OsuError> {
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
            RedisData::Original(user) => user.avatar_url.as_str(),
            RedisData::Archive(user) => user.avatar_url.as_str(),
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

    pub fn peek_stats<F, O>(&self, f: F) -> O
    where
        F: FnOnce(&UserStatistics) -> O,
    {
        let res_opt = match self {
            RedisData::Original(user) => user.statistics.as_ref().map(f),
            RedisData::Archive(user) => user
                .statistics
                .as_ref()
                .map(|stats| f(&stats.deserialize(&mut Infallible).unwrap())),
        };

        res_opt.expect("missing statistics")
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

                let country_code =
                    CountryCodeWrapper::deserialize_with(&user.country_code, &mut Infallible)
                        .unwrap();

                let text = format!(
                    "{name}: {pp}pp (#{global} {country_code}{national})",
                    name = user.username,
                    pp = WithComma::new(stats.pp),
                    global = WithComma::new(stats.global_rank.as_ref().map_or(0, |n| *n)),
                    national = stats.country_rank.as_ref().map_or(0, |n| *n)
                );

                let url = format!("{OSU_BASE}users/{}/{}", user.user_id, user.mode);
                let icon = flag_url(&country_code);

                AuthorBuilder::new(text).url(url).icon_url(icon)
            }
        }
    }
}

// 960 bytes vs 352 bytes
// https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=f9a1d7a3d10469fa29bf1253d4207b75
#[derive(Archive, Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "rkyv", derive(Archive, RkyvDeserialize, RkyvSerialize))]
pub struct User {
    pub avatar_url: String,
    #[with(CountryCodeWrapper)]
    pub country_code: CountryCode,
    #[with(DateTimeWrapper)]
    pub join_date: OffsetDateTime,
    pub kudosu: UserKudosu,
    #[with(Map<DateTimeWrapper>)]
    pub last_visit: Option<OffsetDateTime>,
    pub mode: GameMode,
    pub user_id: u32,
    #[with(UsernameWrapper)]
    pub username: Username,

    pub badges: Vec<Badge>,
    pub follower_count: u32,
    pub graveyard_mapset_count: u32,
    pub guest_mapset_count: u32,
    pub highest_rank: Option<UserHighestRank>,
    pub loved_mapset_count: u32,
    pub mapping_follower_count: u32,
    pub monthly_playcounts: Vec<MonthlyCount>,
    pub rank_history: Vec<u32>,
    pub ranked_mapset_count: u32,
    pub replays_watched_counts: Vec<MonthlyCount>,
    pub scores_first_count: u32,
    pub statistics: Option<UserStatistics>,
    pub pending_mapset_count: u32,
    pub medals: Vec<MedalCompact>,
}

impl From<RosuUser> for User {
    #[inline]
    fn from(user: RosuUser) -> Self {
        let RosuUser {
            avatar_url,
            country_code,
            join_date,
            kudosu,
            last_visit,
            mode,
            user_id,
            username,
            badges,
            follower_count,
            graveyard_mapset_count,
            guest_mapset_count,
            highest_rank,
            loved_mapset_count,
            mapping_follower_count,
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

        Self {
            avatar_url,
            country_code,
            join_date,
            kudosu,
            last_visit,
            mode,
            user_id,
            username,
            badges: badges.unwrap_or_default(),
            follower_count: follower_count.unwrap_or_default(),
            graveyard_mapset_count: graveyard_mapset_count.unwrap_or_default(),
            guest_mapset_count: guest_mapset_count.unwrap_or_default(),
            highest_rank,
            loved_mapset_count: loved_mapset_count.unwrap_or_default(),
            mapping_follower_count: mapping_follower_count.unwrap_or_default(),
            monthly_playcounts: monthly_playcounts.unwrap_or_default(),
            rank_history: rank_history.unwrap_or_default(),
            ranked_mapset_count: ranked_mapset_count.unwrap_or_default(),
            replays_watched_counts: replays_watched_counts.unwrap_or_default(),
            scores_first_count: scores_first_count.unwrap_or_default(),
            statistics,
            pending_mapset_count: pending_mapset_count.unwrap_or_default(),
            medals: medals.unwrap_or_default(),
        }
    }
}
