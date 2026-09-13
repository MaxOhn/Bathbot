#![expect(clippy::unit_arg, reason = "rkyv resolvers are fine")]

use ::rkyv::{
    Archive, Deserialize, Place, Serialize,
    munge::munge,
    niche::niching::Bool,
    rancor::{Fallible, Source},
    ser::{Allocator, Writer},
    with::{ArchiveWith, Map, MapNiche, NicheInto, SerializeWith},
};
use rosu_v2::prelude::*;
use time::OffsetDateTime;

use crate::rkyv_util::{MapUnwrapOrDefault, time::DateTimeRkyv};

/// The type of a [`MatchmakingPool`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Archive, Deserialize, Serialize)]
pub enum MatchmakingPoolTypeRkyv {
    /// Quick play.
    QuickPlay,
    /// Ranked play.
    RankedPlay,
    /// Covering `non_exhaustive` pattern
    Other,
}

/// The result of a match in a [`MatchmakingPool`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Archive, Deserialize, Serialize)]
pub enum MatchmakingResultRkyv {
    Win,
    Loss,
    Draw,
}

/// A matchmaking pool users can play in.
#[derive(Clone, Debug, Eq, PartialEq, Archive, Serialize, Deserialize)]
#[rkyv(archived = ArchivedMatchmakingPool, resolver = MatchmakingPoolResolver)]
pub struct MatchmakingPoolRkyv {
    /// Whether the pool is currently active.
    #[rkyv(niche = Bool)]
    pub active: bool,
    /// Display name of the pool.
    pub name: String,
    /// The game mode of the pool.
    pub mode: GameMode,
    /// The type of the pool.
    pub kind: MatchmakingPoolTypeRkyv,
}

const fn convert_pool_type(kind: MatchmakingPoolType) -> MatchmakingPoolTypeRkyv {
    match kind {
        MatchmakingPoolType::QuickPlay => MatchmakingPoolTypeRkyv::QuickPlay,
        MatchmakingPoolType::RankedPlay => MatchmakingPoolTypeRkyv::RankedPlay,
        _ => MatchmakingPoolTypeRkyv::Other,
    }
}

impl ArchiveWith<MatchmakingPool> for MatchmakingPoolRkyv {
    type Archived = ArchivedMatchmakingPool;
    type Resolver = MatchmakingPoolResolver;

    fn resolve_with(pool: &MatchmakingPool, resolver: Self::Resolver, out: Place<Self::Archived>) {
        munge!(let ArchivedMatchmakingPool {
                active,
                name,
                mode,
                kind,
            } = out);

        pool.active.resolve(resolver.active, active);
        pool.name.resolve(resolver.name, name);
        pool.mode.resolve(resolver.mode, mode);
        convert_pool_type(pool.kind).resolve(resolver.kind, kind);
    }
}

impl<S> SerializeWith<MatchmakingPool, S> for MatchmakingPoolRkyv
where
    S: Fallible<Error: Source> + Writer + ?Sized,
{
    fn serialize_with(
        pool: &MatchmakingPool,
        s: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        Ok(MatchmakingPoolResolver {
            active: pool.active.serialize(s)?,
            name: pool.name.serialize(s)?,
            mode: pool.mode.serialize(s)?,
            kind: convert_pool_type(pool.kind).serialize(s)?,
        })
    }
}

/// A single elo history entry of a user in a [`MatchmakingPoolRkyv`].
#[derive(Clone, Debug, Eq, PartialEq, Archive, Serialize, Deserialize)]
#[rkyv(archived = ArchivedMatchmakingUserEloHistory, resolver = MatchmakingUserEloHistoryResolver)]
pub struct MatchmakingUserEloHistoryRkyv {
    /// The elo of the user after the match.
    pub elo_after: i32,
    /// When the entry was created.
    #[rkyv(with = Map<DateTimeRkyv>)]
    pub created_at: Option<OffsetDateTime>,
    /// The result of the match.
    pub result: MatchmakingResultRkyv,
}

const fn convert_result(result: MatchmakingResult) -> MatchmakingResultRkyv {
    match result {
        MatchmakingResult::Win => MatchmakingResultRkyv::Win,
        MatchmakingResult::Loss => MatchmakingResultRkyv::Loss,
        MatchmakingResult::Draw => MatchmakingResultRkyv::Draw,
    }
}

impl ArchiveWith<MatchmakingUserEloHistory> for MatchmakingUserEloHistoryRkyv {
    type Archived = ArchivedMatchmakingUserEloHistory;
    type Resolver = MatchmakingUserEloHistoryResolver;

    fn resolve_with(
        history: &MatchmakingUserEloHistory,
        resolver: Self::Resolver,
        out: Place<Self::Archived>,
    ) {
        munge!(let ArchivedMatchmakingUserEloHistory {
                elo_after,
                created_at,
                result
            } = out);

        history.elo_after.resolve(resolver.elo_after, elo_after);
        Map::<DateTimeRkyv>::resolve_with(&history.created_at, resolver.created_at, created_at);
        convert_result(history.result).resolve(resolver.result, result);
    }
}

impl<S> SerializeWith<MatchmakingUserEloHistory, S> for MatchmakingUserEloHistoryRkyv
where
    S: Fallible + ?Sized,
{
    fn serialize_with(
        history: &MatchmakingUserEloHistory,
        s: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        Ok(MatchmakingUserEloHistoryResolver {
            elo_after: history.elo_after.serialize(s)?,
            created_at: Map::<DateTimeRkyv>::serialize_with(&history.created_at, s)?,
            result: convert_result(history.result).serialize(s)?,
        })
    }
}

/// The ranked play stats of a user in a single pool.
#[derive(Clone, Debug, PartialEq, Archive, Serialize, Deserialize)]
#[rkyv(archived = ArchivedMatchmakingUserStats, resolver = MatchmakingUserStatsResolver)]
pub struct MatchmakingUserStatsRkyv {
    /// The number of first placements.
    pub first_placements: u32,
    /// Whether the rating of the user is still provisional.
    pub is_rating_provisional: bool,
    /// The number of plays.
    pub plays: u32,
    /// The rank of the user in the pool (`1` being the highest).
    pub rank: u32,
    /// The percentile rank of the user in the pool (`0.0` to `1.0`).
    pub rank_percent: f64,
    /// The current rating of the user in the pool.
    pub rating: i32,
    /// The total points of the user in the pool.
    pub total_points: i32,
    /// Unique identifier of the user.
    pub user_id: u32,

    /// The pool the stats belong to.
    #[rkyv(with = NicheInto<Bool>)]
    pub pool: Option<MatchmakingPoolRkyv>,
    /// Recent elo history entries of the user in the pool.
    pub recent_history: Vec<MatchmakingUserEloHistoryRkyv>,
}

impl ArchiveWith<MatchmakingUserStats> for MatchmakingUserStatsRkyv {
    type Archived = ArchivedMatchmakingUserStats;
    type Resolver = MatchmakingUserStatsResolver;

    fn resolve_with(
        stats: &MatchmakingUserStats,
        resolver: Self::Resolver,
        out: Place<Self::Archived>,
    ) {
        munge!(let ArchivedMatchmakingUserStats {
                first_placements,
                is_rating_provisional,
                plays,
                rank,
                rank_percent,
                rating,
                total_points,
                user_id,
                pool,
                recent_history,
            } = out);

        stats
            .first_placements
            .resolve(resolver.first_placements, first_placements);
        stats
            .is_rating_provisional
            .resolve(resolver.is_rating_provisional, is_rating_provisional);
        stats.plays.resolve(resolver.plays, plays);
        stats.rank.resolve(resolver.rank, rank);
        stats
            .rank_percent
            .resolve(resolver.rank_percent, rank_percent);
        stats.rating.resolve(resolver.rating, rating);
        stats
            .total_points
            .resolve(resolver.total_points, total_points);
        stats.user_id.resolve(resolver.user_id, user_id);
        MapNiche::<MatchmakingPoolRkyv, Bool>::resolve_with(&stats.pool, resolver.pool, pool);
        MapUnwrapOrDefault::<MatchmakingUserEloHistoryRkyv>::resolve_with(
            &stats.recent_history,
            resolver.recent_history,
            recent_history,
        );
    }
}

impl<S> SerializeWith<MatchmakingUserStats, S> for MatchmakingUserStatsRkyv
where
    S: Fallible<Error: Source> + Writer + Allocator + ?Sized,
{
    fn serialize_with(
        stats: &MatchmakingUserStats,
        s: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        Ok(MatchmakingUserStatsResolver {
            first_placements: stats.first_placements.serialize(s)?,
            is_rating_provisional: stats.is_rating_provisional.serialize(s)?,
            plays: stats.plays.serialize(s)?,
            rank: stats.rank.serialize(s)?,
            rank_percent: stats.rank_percent.serialize(s)?,
            rating: stats.rating.serialize(s)?,
            total_points: stats.total_points.serialize(s)?,
            user_id: stats.user_id.serialize(s)?,
            pool: Map::<MatchmakingPoolRkyv>::serialize_with(&stats.pool, s)?,
            recent_history: MapUnwrapOrDefault::<MatchmakingUserEloHistoryRkyv>::serialize_with(
                &stats.recent_history,
                s,
            )?,
        })
    }
}
