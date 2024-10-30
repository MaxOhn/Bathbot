use ::rosu_v2::model::user::{
    Badge as RosuBadge, GradeCounts as RosuGradeCounts, MedalCompact as RosuMedalCompact,
    MonthlyCount as RosuMonthlyCount, UserExtended, UserHighestRank as RosuUserHighestRank,
    UserKudosu as RosuUserKudosu, UserLevel as RosuUserLevel, UserStatistics as RosuUserStatistics,
};
use bathbot_util::osu::UserStats;
use rkyv::{rancor::Source, with::Map, Archive, Serialize};
use rosu_v2::prelude::{CountryCode, GameMode, Username};
use time::{Date, OffsetDateTime};

use crate::rkyv_util::{
    time::{DateRkyv, DateTimeRkyv},
    DerefAsString, MapUnwrapOrDefault, UnwrapOrDefault,
};

#[derive(Archive, Serialize)]
#[rkyv(remote = RosuBadge)]
pub struct Badge {
    #[rkyv(with = DateTimeRkyv)]
    pub awarded_at: OffsetDateTime,
    pub description: String,
    pub image_url: String,
    pub url: String,
}

#[derive(Copy, Clone, Debug, PartialEq, Archive, Serialize)]
#[rkyv(remote = RosuGradeCounts, derive(Copy, Clone))]
pub struct GradeCounts {
    pub ss: i32,
    pub ssh: i32,
    pub s: i32,
    pub sh: i32,
    pub a: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Archive, Serialize)]
#[rkyv(remote = RosuMedalCompact)]
pub struct MedalCompact {
    #[rkyv(with = DateTimeRkyv)]
    pub achieved_at: OffsetDateTime,
    pub medal_id: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Archive, Serialize)]
#[rkyv(remote = RosuMonthlyCount)]
pub struct MonthlyCount {
    #[rkyv(with = DateRkyv)]
    pub start_date: Date,
    pub count: i32,
}

#[derive(Archive, Clone, Debug, PartialEq, Serialize)]
#[rkyv(remote = UserExtended)]
pub struct User {
    pub avatar_url: String,
    #[rkyv(with = DerefAsString)]
    pub country_code: CountryCode,
    #[rkyv(with = DateTimeRkyv)]
    pub join_date: OffsetDateTime,
    #[rkyv(with = UserKudosu)]
    pub kudosu: RosuUserKudosu,
    #[rkyv(with = Map<DateTimeRkyv>)]
    pub last_visit: Option<OffsetDateTime>,
    pub mode: GameMode,
    pub user_id: u32,
    #[rkyv(with = DerefAsString)]
    pub username: Username,

    #[rkyv(with = MapUnwrapOrDefault<Badge>)]
    pub badges: Option<Vec<RosuBadge>>,
    #[rkyv(with = Map<UserHighestRank>)]
    pub highest_rank: Option<RosuUserHighestRank>,
    #[rkyv(with = UnwrapOrDefault)]
    pub follower_count: Option<u32>,
    #[rkyv(with = UnwrapOrDefault)]
    pub graveyard_mapset_count: Option<u32>,
    #[rkyv(with = UnwrapOrDefault)]
    pub guest_mapset_count: Option<u32>,
    #[rkyv(with = UnwrapOrDefault)]
    pub loved_mapset_count: Option<u32>,
    #[rkyv(with = UnwrapOrDefault)]
    pub mapping_follower_count: Option<u32>,
    #[rkyv(with = UnwrapOrDefault)]
    pub ranked_mapset_count: Option<u32>,
    #[rkyv(with = UnwrapOrDefault)]
    pub scores_first_count: Option<u32>,
    #[rkyv(with = UnwrapOrDefault)]
    pub pending_mapset_count: Option<u32>,
    #[rkyv(with = MapUnwrapOrDefault<MonthlyCount>)]
    pub monthly_playcounts: Option<Vec<RosuMonthlyCount>>,
    #[rkyv(with = UnwrapOrDefault)]
    pub rank_history: Option<Vec<u32>>,
    #[rkyv(with = MapUnwrapOrDefault<MonthlyCount>)]
    pub replays_watched_counts: Option<Vec<RosuMonthlyCount>>,
    #[rkyv(with = Map<UserStatistics>)]
    pub statistics: Option<RosuUserStatistics>,
    #[rkyv(with = MapUnwrapOrDefault<MedalCompact>)]
    pub medals: Option<Vec<RosuMedalCompact>>,
}

#[derive(Archive, Serialize)]
#[rkyv(remote = RosuUserHighestRank, derive(Clone))]
pub struct UserHighestRank {
    pub rank: u32,
    #[rkyv(with = DateTimeRkyv)]
    pub updated_at: OffsetDateTime,
}

impl ArchivedUserHighestRank {
    pub fn try_deserialize<E: Source>(&self) -> Result<RosuUserHighestRank, E> {
        Ok(RosuUserHighestRank {
            rank: self.rank.to_native(),
            updated_at: self.updated_at.try_deserialize()?,
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Archive, Serialize)]
#[rkyv(remote = RosuUserKudosu, derive(Copy, Clone))]
pub struct UserKudosu {
    pub available: i32,
    pub total: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Archive, Serialize)]
#[rkyv(remote = RosuUserLevel, derive(Copy, Clone))]
pub struct UserLevel {
    pub current: u32,
    pub progress: u32,
}

impl ArchivedUserLevel {
    pub fn float(&self) -> f32 {
        self.current.to_native() as f32 + self.progress.to_native() as f32 / 100.0
    }
}

#[derive(Clone, Debug, PartialEq, Archive, Serialize)]
#[rkyv(remote = RosuUserStatistics, derive(Clone))]
pub struct UserStatistics {
    #[rkyv(with = GradeCounts)]
    pub grade_counts: RosuGradeCounts,
    #[rkyv(with = UserLevel)]
    pub level: RosuUserLevel,
    pub accuracy: f32,
    #[rkyv(with = UnwrapOrDefault)]
    pub country_rank: Option<u32>,
    #[rkyv(with = UnwrapOrDefault)]
    pub global_rank: Option<u32>,
    pub max_combo: u32,
    pub playcount: u32,
    pub playtime: u32,
    pub pp: f32,
    pub replays_watched: u32,
    pub ranked_score: u64,
    pub total_hits: u64,
    pub total_score: u64,
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
