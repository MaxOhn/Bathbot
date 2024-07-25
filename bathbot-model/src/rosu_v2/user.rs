use ::rosu_v2::model::user::{
    Badge as RosuBadge, GradeCounts as RosuGradeCounts, MedalCompact as RosuMedalCompact,
    MonthlyCount as RosuMonthlyCount, UserExtended as RosuUserExtended,
    UserHighestRank as RosuUserHighestRank, UserKudosu as RosuUserKudosu,
    UserLevel as RosuUserLevel, UserStatistics as RosuUserStatistics,
};
use bathbot_util::osu::UserStats;
use rkyv::{
    with::{ArchiveWith, CopyOptimize, DeserializeWith, Map},
    Archive, Deserialize, Serialize,
};
use rkyv_with::{ArchiveWith, DeserializeWith};
use rosu_v2::prelude::{CountryCode, GameMode, Username};
use time::{Date, OffsetDateTime};

use crate::{
    rkyv_util::{
        time::{DateRkyv, DateTimeRkyv},
        DerefAsBox, DerefAsString, UnwrapOrDefault,
    },
    Either,
};

#[derive(Archive, ArchiveWith)]
#[archive_with(from(RosuBadge))]
pub struct Badge {
    #[with(DateTimeRkyv)]
    pub awarded_at: OffsetDateTime,
    #[archive_with(from(String), via(DerefAsBox))]
    pub description: Box<str>,
    #[archive_with(from(String), via(DerefAsBox))]
    pub image_url: Box<str>,
    #[archive_with(from(String), via(DerefAsBox))]
    pub url: Box<str>,
}

#[derive(Archive, ArchiveWith, DeserializeWith)]
#[archive(as = "RosuGradeCounts")]
#[archive_with(from(RosuGradeCounts))]
pub struct GradeCounts {
    pub ss: i32,
    pub ssh: i32,
    pub s: i32,
    pub sh: i32,
    pub a: i32,
}

#[derive(Archive, ArchiveWith, DeserializeWith)]
#[archive_with(from(RosuMedalCompact))]
pub struct MedalCompact {
    #[with(DateTimeRkyv)]
    pub achieved_at: OffsetDateTime,
    pub medal_id: u32,
}

#[derive(Archive, ArchiveWith, DeserializeWith)]
#[archive_with(from(RosuMonthlyCount))]
pub struct MonthlyCount {
    #[with(DateRkyv)]
    pub start_date: Date,
    pub count: i32,
}

// 960 bytes vs 336 bytes
// https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=de33384e9ec7b7f1be86034b4e701700
#[derive(Archive, Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct User {
    pub avatar_url: Box<str>,
    #[with(DerefAsString)]
    pub country_code: CountryCode,
    #[with(DateTimeRkyv)]
    pub join_date: OffsetDateTime,
    #[with(UserKudosu)]
    pub kudosu: RosuUserKudosu,
    #[with(Map<DateTimeRkyv>)]
    pub last_visit: Option<OffsetDateTime>,
    pub mode: GameMode,
    pub user_id: u32,
    #[with(DerefAsString)]
    pub username: Username,

    #[with(Map<Badge>)]
    pub badges: Vec<RosuBadge>,
    pub follower_count: u32,
    pub graveyard_mapset_count: u32,
    pub guest_mapset_count: u32,
    #[with(Map<UserHighestRank>)]
    pub highest_rank: Option<RosuUserHighestRank>,
    pub loved_mapset_count: u32,
    pub mapping_follower_count: u32,
    #[with(Map<MonthlyCount>)]
    pub monthly_playcounts: Vec<RosuMonthlyCount>,
    #[with(CopyOptimize)]
    pub rank_history: Box<[u32]>,
    pub ranked_mapset_count: u32,
    #[with(Map<MonthlyCount>)]
    pub replays_watched_counts: Vec<RosuMonthlyCount>,
    pub scores_first_count: u32,
    #[with(Map<UserStatistics>)]
    pub statistics: Option<RosuUserStatistics>,
    pub pending_mapset_count: u32,
    #[with(Map<MedalCompact>)]
    pub medals: Vec<RosuMedalCompact>,
}

impl From<RosuUserExtended> for User {
    #[inline]
    fn from(user: RosuUserExtended) -> Self {
        let RosuUserExtended {
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
            avatar_url: avatar_url.into_boxed_str(),
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
            rank_history: rank_history.unwrap_or_default().into_boxed_slice(),
            ranked_mapset_count: ranked_mapset_count.unwrap_or_default(),
            replays_watched_counts: replays_watched_counts.unwrap_or_default(),
            scores_first_count: scores_first_count.unwrap_or_default(),
            statistics,
            pending_mapset_count: pending_mapset_count.unwrap_or_default(),
            medals: medals.unwrap_or_default(),
        }
    }
}
#[derive(Archive, ArchiveWith, DeserializeWith)]
#[archive_with(from(RosuUserHighestRank))]
pub struct UserHighestRank {
    pub rank: u32,
    #[with(DateTimeRkyv)]
    pub updated_at: OffsetDateTime,
}

#[derive(Archive, ArchiveWith)]
#[archive(as = "RosuUserKudosu")]
#[archive_with(from(RosuUserKudosu))]
pub struct UserKudosu {
    pub available: i32,
    pub total: i32,
}

#[derive(Archive, ArchiveWith, DeserializeWith)]
#[archive(as = "RosuUserLevel")]
#[archive_with(from(RosuUserLevel))]
pub struct UserLevel {
    pub current: u32,
    pub progress: u32,
}

#[derive(Archive, ArchiveWith, Clone, Deserialize)]
#[archive(as = "Self")]
#[archive_with(from(RosuUserStatistics))]
pub struct UserStatistics {
    pub accuracy: f32,
    #[archive_with(from(Option<u32>), via(UnwrapOrDefault))]
    pub country_rank: u32,
    #[archive_with(from(Option<u32>), via(UnwrapOrDefault))]
    pub global_rank: u32,
    #[with(GradeCounts)]
    pub grade_counts: RosuGradeCounts,
    #[with(UserLevel)]
    pub level: RosuUserLevel,
    pub max_combo: u32,
    pub playcount: u32,
    pub playtime: u32,
    pub pp: f32,
    pub ranked_score: u64,
    pub replays_watched: u32,
    pub total_hits: u64,
    pub total_score: u64,
}

impl From<RosuUserStatistics> for UserStatistics {
    #[inline]
    fn from(stats: RosuUserStatistics) -> Self {
        Self {
            accuracy: stats.accuracy,
            country_rank: stats.country_rank.unwrap_or(0),
            global_rank: stats.global_rank.unwrap_or(0),
            grade_counts: stats.grade_counts,
            level: stats.level,
            max_combo: stats.max_combo,
            playcount: stats.playcount,
            playtime: stats.playtime,
            pp: stats.pp,
            ranked_score: stats.ranked_score,
            replays_watched: stats.replays_watched,
            total_hits: stats.total_hits,
            total_score: stats.total_score,
        }
    }
}

impl From<&RosuUserStatistics> for UserStatistics {
    #[inline]
    fn from(stats: &RosuUserStatistics) -> Self {
        Self {
            accuracy: stats.accuracy,
            country_rank: stats.country_rank.unwrap_or(0),
            global_rank: stats.global_rank.unwrap_or(0),
            grade_counts: stats.grade_counts.clone(),
            level: stats.level,
            max_combo: stats.max_combo,
            playcount: stats.playcount,
            playtime: stats.playtime,
            pp: stats.pp,
            ranked_score: stats.ranked_score,
            replays_watched: stats.replays_watched,
            total_hits: stats.total_hits,
            total_score: stats.total_score,
        }
    }
}

pub type StatsWrapper<'s> = Either<&'s RosuUserStatistics, &'s UserStatistics>;

impl StatsWrapper<'_> {
    pub fn to_owned(&self) -> UserStatistics {
        match *self {
            Self::Left(stats) => UserStatistics::from(stats),
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

    pub fn grade_counts(&self) -> &RosuGradeCounts {
        match self {
            Self::Left(stats) => &stats.grade_counts,
            Self::Right(stats) => &stats.grade_counts,
        }
    }

    pub fn level(&self) -> RosuUserLevel {
        match self {
            Either::Left(stats) => stats.level,
            Either::Right(stats) => stats.level,
        }
    }

    pub fn max_combo(&self) -> u32 {
        match self {
            Either::Left(stats) => stats.max_combo,
            Either::Right(stats) => stats.max_combo,
        }
    }

    pub fn playcount(&self) -> u32 {
        match self {
            Either::Left(stats) => stats.playcount,
            Either::Right(stats) => stats.playcount,
        }
    }

    pub fn playtime(&self) -> u32 {
        match self {
            Either::Left(stats) => stats.playtime,
            Either::Right(stats) => stats.playtime,
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
            Either::Left(stats) => stats.replays_watched,
            Either::Right(stats) => stats.replays_watched,
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
