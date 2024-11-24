use ::rosu_v2::model::user::{
    Badge, GradeCounts, MedalCompact, MonthlyCount, UserExtended, UserHighestRank, UserKudosu,
    UserLevel, UserStatistics,
};
use bathbot_util::osu::UserStats;
use rkyv::{
    munge::munge,
    rancor::{Fallible, Source},
    rend::u32_le,
    ser::{Allocator, Writer},
    with::{ArchiveWith, InlineAsBox, Map, SerializeWith},
    Archive, Deserialize, Place, Serialize,
};
use rosu_v2::prelude::{CountryCode, GameMode, Username};
use time::{Date, OffsetDateTime};

use crate::{
    rkyv_util::{
        time::{DateRkyv, DateTimeRkyv},
        DerefAsBox, DerefAsString, MapUnwrapOrDefault, UnwrapOrDefault,
    },
    Either,
};

#[derive(Archive, Serialize)]
#[rkyv(remote = Badge, archived = ArchivedBadge)]
pub struct BadgeRkyv {
    #[rkyv(with = DateTimeRkyv)]
    pub awarded_at: OffsetDateTime,
    pub description: String,
    pub image_url: String,
    pub url: String,
}

#[derive(Archive, Serialize, Deserialize)]
#[rkyv(remote = GradeCounts, archived = ArchivedGradeCounts)]
pub struct GradeCountsRkyv {
    pub ss: i32,
    pub ssh: i32,
    pub s: i32,
    pub sh: i32,
    pub a: i32,
}

impl From<GradeCountsRkyv> for GradeCounts {
    fn from(counts: GradeCountsRkyv) -> Self {
        Self {
            ss: counts.ss,
            ssh: counts.ssh,
            s: counts.s,
            sh: counts.sh,
            a: counts.a,
        }
    }
}

#[derive(Archive, Serialize, Deserialize)]
#[rkyv(remote = MedalCompact, archived = ArchivedMedalCompact)]
pub struct MedalCompactRkyv {
    #[rkyv(with = DateTimeRkyv)]
    pub achieved_at: OffsetDateTime,
    pub medal_id: u32,
}

impl From<MedalCompactRkyv> for MedalCompact {
    fn from(medal: MedalCompactRkyv) -> Self {
        Self {
            achieved_at: medal.achieved_at,
            medal_id: medal.medal_id,
        }
    }
}

#[derive(Archive, Serialize, Deserialize)]
#[rkyv(remote = MonthlyCount, archived = ArchivedMonthlyCount)]
pub struct MonthlyCountRkyv {
    #[rkyv(with = DateRkyv)]
    pub start_date: Date,
    pub count: i32,
}

impl From<MonthlyCountRkyv> for MonthlyCount {
    fn from(count: MonthlyCountRkyv) -> Self {
        Self {
            start_date: count.start_date,
            count: count.count,
        }
    }
}

#[derive(Archive, Serialize)]
pub struct User {
    pub avatar_url: Box<str>,
    #[rkyv(with = DerefAsString)]
    pub country_code: CountryCode,
    #[rkyv(with = DateTimeRkyv)]
    pub join_date: OffsetDateTime,
    #[rkyv(with = UserKudosuRkyv)]
    pub kudosu: UserKudosu,
    #[rkyv(with = Map<DateTimeRkyv>)]
    pub last_visit: Option<OffsetDateTime>,
    pub mode: GameMode,
    pub user_id: u32,
    #[rkyv(with = DerefAsString)]
    pub username: Username,

    #[rkyv(with = Map<BadgeRkyv>)]
    pub badges: Vec<Badge>,
    pub follower_count: u32,
    pub graveyard_mapset_count: u32,
    pub guest_mapset_count: u32,
    #[rkyv(with = Map<UserHighestRankRkyv>)]
    pub highest_rank: Option<UserHighestRank>,
    pub loved_mapset_count: u32,
    pub mapping_follower_count: u32,
    #[rkyv(with = Map<MonthlyCountRkyv>)]
    pub monthly_playcounts: Vec<MonthlyCount>,
    pub rank_history: Box<[u32]>,
    pub ranked_mapset_count: u32,
    #[rkyv(with = Map<MonthlyCountRkyv>)]
    pub replays_watched_counts: Vec<MonthlyCount>,
    pub scores_first_count: u32,
    #[rkyv(with = Map<UserStatisticsRkyv>)]
    pub statistics: Option<UserStatistics>,
    pub pending_mapset_count: u32,
    #[rkyv(with = Map<MedalCompactRkyv>)]
    pub medals: Vec<MedalCompact>,
}

impl ArchiveWith<UserExtended> for User {
    type Archived = ArchivedUser;
    type Resolver = UserResolver;

    #[allow(clippy::unit_arg)]
    fn resolve_with(user: &UserExtended, resolver: Self::Resolver, out: Place<Self::Archived>) {
        munge!(let ArchivedUser {
            avatar_url,
            country_code,
            join_date,
            kudosu,
            last_visit,
            mode,
            user_id,
            username,
            badges,
            highest_rank,
            follower_count,
            graveyard_mapset_count,
            guest_mapset_count,
            loved_mapset_count,
            mapping_follower_count,
            ranked_mapset_count,
            scores_first_count,
            pending_mapset_count,
            monthly_playcounts,
            rank_history,
            replays_watched_counts,
            statistics,
            medals
        } = out);

        DerefAsBox::resolve_with(&user.avatar_url, resolver.avatar_url, avatar_url);
        DerefAsString::resolve_with(&user.country_code, resolver.country_code, country_code);
        DateTimeRkyv::resolve_with(&user.join_date, resolver.join_date, join_date);
        UserKudosuRkyv::resolve_with(&user.kudosu, resolver.kudosu, kudosu);
        Map::<DateTimeRkyv>::resolve_with(&user.last_visit, resolver.last_visit, last_visit);
        user.mode.resolve(resolver.mode, mode);
        user.user_id.resolve(resolver.user_id, user_id);
        DerefAsString::resolve_with(&user.username, resolver.username, username);
        MapUnwrapOrDefault::<BadgeRkyv>::resolve_with(&user.badges, resolver.badges, badges);
        UnwrapOrDefault::resolve_with(
            &user.follower_count,
            resolver.follower_count,
            follower_count,
        );
        UnwrapOrDefault::resolve_with(
            &user.graveyard_mapset_count,
            resolver.graveyard_mapset_count,
            graveyard_mapset_count,
        );
        UnwrapOrDefault::resolve_with(
            &user.guest_mapset_count,
            resolver.guest_mapset_count,
            guest_mapset_count,
        );
        Map::<UserHighestRankRkyv>::resolve_with(
            &user.highest_rank,
            resolver.highest_rank,
            highest_rank,
        );
        UnwrapOrDefault::resolve_with(
            &user.loved_mapset_count,
            resolver.loved_mapset_count,
            loved_mapset_count,
        );
        UnwrapOrDefault::resolve_with(
            &user.mapping_follower_count,
            resolver.mapping_follower_count,
            mapping_follower_count,
        );
        UnwrapOrDefault::resolve_with(
            &user.ranked_mapset_count,
            resolver.ranked_mapset_count,
            ranked_mapset_count,
        );
        UnwrapOrDefault::resolve_with(
            &user.scores_first_count,
            resolver.scores_first_count,
            scores_first_count,
        );
        UnwrapOrDefault::resolve_with(
            &user.pending_mapset_count,
            resolver.pending_mapset_count,
            pending_mapset_count,
        );
        MapUnwrapOrDefault::<MonthlyCountRkyv>::resolve_with(
            &user.monthly_playcounts,
            resolver.monthly_playcounts,
            monthly_playcounts,
        );
        InlineAsBox::resolve_with(
            &user.rank_history.as_deref().unwrap_or_default(),
            resolver.rank_history,
            rank_history,
        );
        MapUnwrapOrDefault::<MonthlyCountRkyv>::resolve_with(
            &user.replays_watched_counts,
            resolver.replays_watched_counts,
            replays_watched_counts,
        );
        Map::<UserStatisticsRkyv>::resolve_with(&user.statistics, resolver.statistics, statistics);
        MapUnwrapOrDefault::<MedalCompactRkyv>::resolve_with(&user.medals, resolver.medals, medals);
    }
}

impl<S: Fallible<Error: Source> + Writer + Allocator + ?Sized> SerializeWith<UserExtended, S>
    for User
{
    fn serialize_with(user: &UserExtended, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(UserResolver {
            avatar_url: DerefAsBox::serialize_with(&user.avatar_url, serializer)?,
            country_code: DerefAsString::serialize_with(&user.country_code, serializer)?,
            join_date: DateTimeRkyv::serialize_with(&user.join_date, serializer)?,
            kudosu: UserKudosuRkyv::serialize_with(&user.kudosu, serializer)?,
            last_visit: Map::<DateTimeRkyv>::serialize_with(&user.last_visit, serializer)?,
            mode: user.mode.serialize(serializer)?,
            user_id: user.user_id.serialize(serializer)?,
            username: DerefAsString::serialize_with(&user.username, serializer)?,
            badges: MapUnwrapOrDefault::<BadgeRkyv>::serialize_with(&user.badges, serializer)?,
            highest_rank: Map::<UserHighestRankRkyv>::serialize_with(
                &user.highest_rank,
                serializer,
            )?,
            follower_count: UnwrapOrDefault::serialize_with(&user.follower_count, serializer)?,
            graveyard_mapset_count: UnwrapOrDefault::serialize_with(
                &user.graveyard_mapset_count,
                serializer,
            )?,
            guest_mapset_count: UnwrapOrDefault::serialize_with(
                &user.guest_mapset_count,
                serializer,
            )?,
            loved_mapset_count: UnwrapOrDefault::serialize_with(
                &user.loved_mapset_count,
                serializer,
            )?,
            mapping_follower_count: UnwrapOrDefault::serialize_with(
                &user.mapping_follower_count,
                serializer,
            )?,
            ranked_mapset_count: UnwrapOrDefault::serialize_with(
                &user.ranked_mapset_count,
                serializer,
            )?,
            scores_first_count: UnwrapOrDefault::serialize_with(
                &user.scores_first_count,
                serializer,
            )?,
            pending_mapset_count: UnwrapOrDefault::serialize_with(
                &user.pending_mapset_count,
                serializer,
            )?,
            monthly_playcounts: MapUnwrapOrDefault::<MonthlyCountRkyv>::serialize_with(
                &user.monthly_playcounts,
                serializer,
            )?,
            rank_history: InlineAsBox::serialize_with(
                &user.rank_history.as_deref().unwrap_or_default(),
                serializer,
            )?,
            replays_watched_counts: MapUnwrapOrDefault::<MonthlyCountRkyv>::serialize_with(
                &user.replays_watched_counts,
                serializer,
            )?,
            statistics: Map::<UserStatisticsRkyv>::serialize_with(&user.statistics, serializer)?,
            medals: MapUnwrapOrDefault::<MedalCompactRkyv>::serialize_with(
                &user.medals,
                serializer,
            )?,
        })
    }
}

impl From<UserExtended> for User {
    fn from(user: UserExtended) -> Self {
        Self {
            avatar_url: user.avatar_url.into_boxed_str(),
            country_code: user.country_code,
            join_date: user.join_date,
            kudosu: user.kudosu,
            last_visit: user.last_visit,
            mode: user.mode,
            user_id: user.user_id,
            username: user.username,
            badges: user.badges.unwrap_or_default(),
            highest_rank: user.highest_rank,
            follower_count: user.follower_count.unwrap_or_default(),
            graveyard_mapset_count: user.graveyard_mapset_count.unwrap_or_default(),
            guest_mapset_count: user.guest_mapset_count.unwrap_or_default(),
            loved_mapset_count: user.loved_mapset_count.unwrap_or_default(),
            mapping_follower_count: user.mapping_follower_count.unwrap_or_default(),
            ranked_mapset_count: user.ranked_mapset_count.unwrap_or_default(),
            scores_first_count: user.scores_first_count.unwrap_or_default(),
            pending_mapset_count: user.pending_mapset_count.unwrap_or_default(),
            monthly_playcounts: user.monthly_playcounts.unwrap_or_default(),
            rank_history: user
                .rank_history
                .map_or(Box::default(), Vec::into_boxed_slice),
            replays_watched_counts: user.replays_watched_counts.unwrap_or_default(),
            statistics: user.statistics,
            medals: user.medals.unwrap_or_default(),
        }
    }
}

#[derive(Archive, Serialize)]
#[rkyv(remote = UserHighestRank, archived = ArchivedUserHighestRank, derive(Clone))]
pub struct UserHighestRankRkyv {
    pub rank: u32,
    #[rkyv(with = DateTimeRkyv)]
    pub updated_at: OffsetDateTime,
}

impl ArchivedUserHighestRank {
    pub fn try_deserialize<E: Source>(&self) -> Result<UserHighestRank, E> {
        Ok(UserHighestRank {
            rank: self.rank.to_native(),
            updated_at: self.updated_at.try_deserialize()?,
        })
    }
}

#[derive(Archive, Serialize)]
#[rkyv(remote = UserKudosu, archived = ArchivedUserKudosu, derive(Copy, Clone))]
pub struct UserKudosuRkyv {
    pub available: i32,
    pub total: i32,
}

#[derive(Archive, Serialize, Deserialize)]
#[rkyv(remote = UserLevel, archived = ArchivedUserLevel)]
pub struct UserLevelRkyv {
    pub current: u32,
    pub progress: u32,
}

impl From<UserLevelRkyv> for UserLevel {
    fn from(level: UserLevelRkyv) -> Self {
        Self {
            current: level.current,
            progress: level.progress,
        }
    }
}

impl ArchivedUserLevel {
    pub fn float(&self) -> f32 {
        self.current.to_native() as f32 + self.progress.to_native() as f32 / 100.0
    }
}

#[derive(Clone, Debug, PartialEq, Archive, Serialize, Deserialize)]
#[rkyv(archived = ArchivedUserStatistics)]
pub struct UserStatisticsRkyv {
    #[rkyv(with = GradeCountsRkyv)]
    pub grade_counts: GradeCounts,
    #[rkyv(with = UserLevelRkyv)]
    pub level: UserLevel,
    pub accuracy: f32,
    pub country_rank: u32,
    pub global_rank: u32,
    pub max_combo: u32,
    pub playcount: u32,
    pub playtime: u32,
    pub pp: f32,
    pub replays_watched: u32,
    pub ranked_score: u64,
    pub total_hits: u64,
    pub total_score: u64,
}

impl ArchiveWith<UserStatistics> for UserStatisticsRkyv {
    type Archived = ArchivedUserStatistics;
    type Resolver = UserStatisticsRkyvResolver;

    #[allow(clippy::unit_arg)]
    fn resolve_with(stats: &UserStatistics, resolver: Self::Resolver, out: Place<Self::Archived>) {
        munge!(let ArchivedUserStatistics {
            grade_counts,
            level,
            accuracy,
            country_rank,
            global_rank,
            max_combo,
            playcount,
            playtime,
            pp,
            replays_watched,
            ranked_score,
            total_hits,
            total_score,
        } = out);

        GradeCountsRkyv::resolve_with(&stats.grade_counts, resolver.grade_counts, grade_counts);
        UserLevelRkyv::resolve_with(&stats.level, resolver.level, level);
        stats.accuracy.resolve(resolver.accuracy, accuracy);
        UnwrapOrDefault::resolve_with(&stats.country_rank, resolver.country_rank, country_rank);
        UnwrapOrDefault::resolve_with(&stats.global_rank, resolver.global_rank, global_rank);
        stats.max_combo.resolve(resolver.max_combo, max_combo);
        stats.playcount.resolve(resolver.playcount, playcount);
        stats.playtime.resolve(resolver.playtime, playtime);
        stats.pp.resolve(resolver.pp, pp);
        stats
            .replays_watched
            .resolve(resolver.replays_watched, replays_watched);
        stats
            .ranked_score
            .resolve(resolver.ranked_score, ranked_score);
        stats.total_hits.resolve(resolver.total_hits, total_hits);
        stats.total_score.resolve(resolver.total_score, total_score);
    }
}

impl<S: Fallible + ?Sized> SerializeWith<UserStatistics, S> for UserStatisticsRkyv {
    fn serialize_with(
        stats: &UserStatistics,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        Ok(UserStatisticsRkyvResolver {
            grade_counts: GradeCountsRkyv::serialize_with(&stats.grade_counts, serializer)?,
            level: UserLevelRkyv::serialize_with(&stats.level, serializer)?,
            accuracy: stats.accuracy.serialize(serializer)?,
            country_rank: UnwrapOrDefault::serialize_with(&stats.country_rank, serializer)?,
            global_rank: UnwrapOrDefault::serialize_with(&stats.global_rank, serializer)?,
            max_combo: stats.max_combo.serialize(serializer)?,
            playcount: stats.playcount.serialize(serializer)?,
            playtime: stats.playtime.serialize(serializer)?,
            pp: stats.pp.serialize(serializer)?,
            replays_watched: stats.replays_watched.serialize(serializer)?,
            ranked_score: stats.ranked_score.serialize(serializer)?,
            total_hits: stats.total_hits.serialize(serializer)?,
            total_score: stats.total_score.serialize(serializer)?,
        })
    }
}

impl From<UserStatistics> for ArchivedUserStatistics {
    fn from(stats: UserStatistics) -> Self {
        Self {
            grade_counts: ArchivedGradeCounts {
                ss: stats.grade_counts.ss.into(),
                ssh: stats.grade_counts.ssh.into(),
                s: stats.grade_counts.s.into(),
                sh: stats.grade_counts.sh.into(),
                a: stats.grade_counts.a.into(),
            },
            level: ArchivedUserLevel {
                current: stats.level.current.into(),
                progress: stats.level.progress.into(),
            },
            accuracy: stats.accuracy.into(),
            country_rank: u32_le::from_native(stats.country_rank.unwrap_or(0)),
            global_rank: u32_le::from_native(stats.global_rank.unwrap_or(0)),
            max_combo: stats.max_combo.into(),
            playcount: stats.playcount.into(),
            playtime: stats.playtime.into(),
            pp: stats.pp.into(),
            replays_watched: stats.replays_watched.into(),
            ranked_score: stats.ranked_score.into(),
            total_hits: stats.total_hits.into(),
            total_score: stats.total_score.into(),
        }
    }
}

impl From<&UserStatistics> for UserStatisticsRkyv {
    fn from(stats: &UserStatistics) -> Self {
        Self {
            grade_counts: stats.grade_counts.to_owned(),
            level: stats.level,
            accuracy: stats.accuracy,
            country_rank: stats.country_rank.unwrap_or_default(),
            global_rank: stats.global_rank.unwrap_or_default(),
            max_combo: stats.max_combo,
            playcount: stats.playcount,
            playtime: stats.playtime,
            pp: stats.pp,
            replays_watched: stats.replays_watched,
            ranked_score: stats.ranked_score,
            total_hits: stats.total_hits,
            total_score: stats.total_score,
        }
    }
}

impl UserStats for ArchivedUserStatistics {
    fn pp(&self) -> f32 {
        self.pp.to_native()
    }

    fn grade_counts_sum(&self) -> i32 {
        let ArchivedGradeCounts { ss, ssh, s, sh, a } = self.grade_counts;

        ss.to_native() + ssh.to_native() + s.to_native() + sh.to_native() + a.to_native()
    }

    fn playcount(&self) -> u32 {
        self.playcount.to_native()
    }
}

pub type StatsWrapper<'s> = Either<&'s UserStatistics, UserStatisticsRkyv>;

impl StatsWrapper<'_> {
    pub fn to_owned(&self) -> UserStatisticsRkyv {
        match self {
            Self::Left(stats) => UserStatisticsRkyv::from(*stats),
            Self::Right(stats) => stats.to_owned(),
        }
    }

    pub fn accuracy(&self) -> f32 {
        match self {
            Self::Left(stats) => stats.accuracy,
            Self::Right(stats) => stats.accuracy,
        }
    }

    pub fn country_rank(&self) -> u32 {
        match self {
            Self::Left(stats) => stats.country_rank.unwrap_or(0),
            Self::Right(stats) => stats.country_rank,
        }
    }

    pub fn global_rank(&self) -> u32 {
        match self {
            Self::Left(stats) => stats.global_rank.unwrap_or(0),
            Self::Right(stats) => stats.global_rank,
        }
    }

    pub fn grade_counts(&self) -> &GradeCounts {
        match self {
            Self::Left(stats) => &stats.grade_counts,
            Self::Right(stats) => &stats.grade_counts,
        }
    }

    pub fn level(&self) -> UserLevel {
        match self {
            Self::Left(stats) => stats.level,
            Self::Right(stats) => stats.level,
        }
    }

    pub fn max_combo(&self) -> u32 {
        match self {
            Self::Left(stats) => stats.max_combo,
            Self::Right(stats) => stats.max_combo,
        }
    }

    pub fn playcount(&self) -> u32 {
        match self {
            Self::Left(stats) => stats.playcount,
            Self::Right(stats) => stats.playcount,
        }
    }

    pub fn playtime(&self) -> u32 {
        match self {
            Self::Left(stats) => stats.playtime,
            Self::Right(stats) => stats.playtime,
        }
    }

    pub fn pp(&self) -> f32 {
        match self {
            Self::Left(stats) => stats.pp,
            Self::Right(stats) => stats.pp,
        }
    }

    pub fn ranked_score(&self) -> u64 {
        match self {
            Self::Left(stats) => stats.ranked_score,
            Self::Right(stats) => stats.ranked_score,
        }
    }

    pub fn replays_watched(&self) -> u32 {
        match self {
            Self::Left(stats) => stats.replays_watched,
            Self::Right(stats) => stats.replays_watched,
        }
    }

    pub fn total_hits(&self) -> u64 {
        match self {
            Self::Left(stats) => stats.total_hits,
            Self::Right(stats) => stats.total_hits,
        }
    }

    pub fn total_score(&self) -> u64 {
        match self {
            Self::Left(stats) => stats.total_score,
            Self::Right(stats) => stats.total_score,
        }
    }
}

impl UserStats for StatsWrapper<'_> {
    #[inline]
    fn pp(&self) -> f32 {
        self.pp()
    }

    #[inline]
    fn grade_counts_sum(&self) -> i32 {
        let grade_counts = self.grade_counts();

        grade_counts.ssh + grade_counts.ss + grade_counts.sh + grade_counts.s + grade_counts.a
    }

    #[inline]
    fn playcount(&self) -> u32 {
        self.playcount()
    }
}
