macro_rules! map_id {
    ($score:ident) => {
        $score.map.as_ref().map(|map| map.map_id)
    };
}

/// Try to extract an osu! username from the `args`' fields `name` or `discord`
macro_rules! username {
    ($ctx:ident, $orig:ident, $args:ident) => {
        match crate::commands::osu::HasName::username(&$args, &$ctx) {
            crate::commands::osu::UsernameResult::Name(name) => Some(name),
            crate::commands::osu::UsernameResult::None => None,
            crate::commands::osu::UsernameResult::Future(fut) => match fut.await {
                crate::commands::osu::UsernameFutureResult::Name(name) => Some(name),
                crate::commands::osu::UsernameFutureResult::NotLinked(user_id) => {
                    let content = format!("<@{user_id}> is not linked to an osu!profile");

                    return $orig.error(&$ctx, content).await;
                }
                crate::commands::osu::UsernameFutureResult::Err(err) => {
                    let content = crate::util::constants::GENERAL_ISSUE;
                    let _ = $orig.error(&$ctx, content).await;

                    return Err(err);
                }
            },
        }
    };
}

macro_rules! username_ref {
    ($ctx:ident, $orig:ident, $args:ident) => {
        match crate::commands::osu::HasName::username($args, &$ctx) {
            crate::commands::osu::UsernameResult::Name(name) => Some(name),
            crate::commands::osu::UsernameResult::None => None,
            crate::commands::osu::UsernameResult::Future(fut) => match fut.await {
                crate::commands::osu::UsernameFutureResult::Name(name) => Some(name),
                crate::commands::osu::UsernameFutureResult::NotLinked(user_id) => {
                    let content = format!("<@{user_id}> is not linked to an osu!profile");

                    return $orig.error(&$ctx, content).await;
                }
                crate::commands::osu::UsernameFutureResult::Err(err) => {
                    let content = crate::util::constants::GENERAL_ISSUE;
                    let _ = $orig.error(&$ctx, content).await;

                    return Err(err);
                }
            },
        }
    };
}

/// Returns a (Username, GameMode) tuple
macro_rules! name_mode {
    ($ctx:ident, $orig:ident, $args:ident) => {{
        let mode = $args.mode.map(rosu_v2::prelude::GameMode::from);

        if let Some(name) = username!($ctx, $orig, $args) {
            if let Some(mode) = mode {
                (name, mode)
            } else {
                let config = $ctx.user_config($orig.user_id()?).await?;
                let mode = config.mode.unwrap_or(rosu_v2::prelude::GameMode::STD);

                (name, mode)
            }
        } else {
            let config = $ctx.user_config($orig.user_id()?).await?;

            let mode = mode
                .or(config.mode)
                .unwrap_or(rosu_v2::prelude::GameMode::STD);

            match config.into_username() {
                Some(name) => (name, mode),
                None => return crate::commands::osu::require_link(&$ctx, &$orig).await,
            }
        }
    }};
}

use std::{
    borrow::Cow,
    cmp::{Ordering, PartialOrd, Reverse},
    collections::BTreeMap,
    future::Future,
    ops::{AddAssign, Div},
    pin::Pin,
};

use chrono::Utc;
use eyre::Report;
use futures::{future::FutureExt, stream::FuturesUnordered, TryFutureExt, TryStreamExt};
use hashbrown::HashMap;
use rosu_v2::{
    prelude::{
        BeatmapUserScore, Beatmapset, GameMode, GameMods, OsuError, OsuResult, Score, User,
        Username,
    },
    request::GetUserScores,
    Osu,
};
use twilight_interactions::command::{CommandOption, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    core::commands::CommandOrigin,
    custom_client::OsuStatsParams,
    pp::PpCalculator,
    util::{
        numbers::with_comma_int,
        osu::{ModSelection, SortableScore},
        CowUtils,
    },
    BotResult, Context, Error,
};

pub use self::{
    avatar::*, badges::*, bws::*, compare::*, country_top::*, fix::*, graphs::*, leaderboard::*,
    link::*, map::*, map_search::*, mapper::*, match_compare::*, match_costs::*, match_live::*,
    medals::*, most_played::*, nochoke::*, osekai::*, osustats::*, pinned::*, popular::*, pp::*,
    profile::*, rank::*, ranking::*, ratios::*, recent::*, serverleaderboard::*, simulate::*,
    snipe::*, top::*, whatif::*,
};

mod avatar;
mod badges;
mod bws;
mod compare;
mod country_top;
mod fix;
mod graphs;
mod leaderboard;
mod link;
mod map;
mod map_search;
mod mapper;
mod match_compare;
mod match_costs;
mod match_live;
mod medals;
mod most_played;
mod nochoke;
mod osekai;
mod osustats;
mod pinned;
mod popular;
mod pp;
mod profile;
mod rank;
mod ranking;
mod ratios;
mod recent;
mod serverleaderboard;
mod simulate;
mod snipe;
mod top;
mod whatif;

pub trait HasMods {
    fn mods(&self) -> ModsResult;
}

pub enum ModsResult {
    Mods(ModSelection),
    None,
    Invalid,
}

pub trait HasName {
    fn username<'ctx>(&self, ctx: &'ctx Context) -> UsernameResult<'ctx>;
}

pub enum UsernameResult<'ctx> {
    Name(Username),
    None,
    Future(Pin<Box<dyn Future<Output = UsernameFutureResult> + 'ctx + Send>>),
}

pub enum UsernameFutureResult {
    Name(Username),
    NotLinked(Id<UserMarker>),
    Err(Error),
}

enum ErrorType {
    Bot(Error),
    Osu(OsuError),
}

impl From<Error> for ErrorType {
    fn from(e: Error) -> Self {
        Self::Bot(e)
    }
}

impl From<OsuError> for ErrorType {
    fn from(e: OsuError) -> Self {
        Self::Osu(e)
    }
}

async fn get_user(ctx: &Context, user: &UserArgs<'_>) -> OsuResult<User> {
    if let Some(alt_name) = user.whitespaced_name() {
        match ctx.redis().osu_user(user).await {
            Err(OsuError::NotFound) => {
                let user = UserArgs::new(&alt_name, user.mode);

                ctx.redis().osu_user(&user).await
            }
            result => result,
        }
    } else {
        ctx.redis().osu_user(user).await
    }
}

async fn get_beatmap_user_score(
    osu: &Osu,
    map_id: u32,
    user: &UserArgs<'_>,
    mods: Option<GameMods>,
) -> OsuResult<BeatmapUserScore> {
    //* Note: GameMode is not specified
    let mut fut = osu.beatmap_user_score(map_id, user.name);

    if let Some(mods) = mods {
        fut = fut.mods(mods);
    }

    if let Some(alt_name) = user.whitespaced_name() {
        match fut.await {
            // * Note: Could also occure due to an incorrect map id
            // *       or the user not having a score on the map
            Err(OsuError::NotFound) => {
                let user = UserArgs::new(&alt_name, user.mode);
                let mut fut = osu.beatmap_user_score(map_id, user.name);

                if let Some(mods) = mods {
                    fut = fut.mods(mods);
                }

                fut.await
            }
            result => result,
        }
    } else {
        fut.await
    }
}

async fn get_user_and_scores<'c>(
    ctx: &'c Context,
    mut user: UserArgs<'_>,
    scores: &ScoreArgs<'c>,
) -> OsuResult<(User, Vec<Score>)> {
    let redis = ctx.redis();

    if let Some(alt_name) = user.whitespaced_name() {
        match redis.osu_user(&user).await {
            Ok(u) => Ok((u, get_scores(ctx, &user, scores).await?)),
            Err(OsuError::NotFound) => {
                user.name = &alt_name;

                let user_fut = redis.osu_user(&user);
                let scores_fut = get_scores(ctx, &user, scores);

                tokio::try_join!(user_fut, scores_fut)
            }
            Err(err) => Err(err),
        }
    } else {
        let user_fut = redis.osu_user(&user);
        let scores_fut = get_scores(ctx, &user, scores);

        tokio::try_join!(user_fut, scores_fut)
    }
}

async fn get_scores<'c>(
    ctx: &'c Context,
    user: &UserArgs<'_>,
    scores: &ScoreArgs<'c>,
) -> OsuResult<Vec<Score>> {
    let scores_fut = {
        let mut fut = ctx
            .osu()
            .user_scores(user.name)
            .mode(user.mode)
            .limit(scores.limit);

        if let Some(include_fails) = scores.include_fails {
            fut = fut.include_fails(include_fails)
        }

        (scores.fun)(fut)
    };

    let result = if scores.with_combo {
        prepare_scores(ctx, scores_fut).await
    } else {
        scores_fut.await
    };

    if let Err(OsuError::NotFound) = &result {
        // Remove stats of unknown/restricted users so they don't appear in the leaderboard
        if let Err(err) = ctx.psql().remove_osu_user_stats(user.name).await {
            let report = Report::new(err).wrap_err("failed to remove stats of unknown user");
            warn!("{report:?}");
        }
    }

    result
}

pub struct UserArgs<'n> {
    pub name: &'n str,
    pub mode: GameMode,
}

impl<'n> UserArgs<'n> {
    pub fn new(name: &'n str, mode: GameMode) -> Self {
        Self { name, mode }
    }

    /// Try to replace underscores with whitespace.
    fn whitespaced_name(&self) -> Option<String> {
        if self.name.starts_with('_') || self.name.ends_with('_') {
            return None;
        }

        match self.name.cow_replace('_', " ") {
            Cow::Borrowed(_) => None,
            Cow::Owned(name) => Some(name),
        }
    }
}

struct ScoreArgs<'o> {
    fun: fn(GetUserScores<'o>) -> GetUserScores<'o>,
    include_fails: Option<bool>,
    limit: usize,
    with_combo: bool,
}

impl<'o> ScoreArgs<'o> {
    fn top(limit: usize) -> Self {
        Self {
            fun: GetUserScores::best,
            include_fails: None,
            limit,
            with_combo: false,
        }
    }

    fn recent(limit: usize) -> Self {
        Self {
            fun: GetUserScores::recent,
            include_fails: None,
            limit,
            with_combo: false,
        }
    }

    fn include_fails(mut self, include_fails: bool) -> Self {
        self.include_fails = Some(include_fails);

        self
    }

    fn with_combo(mut self) -> Self {
        self.with_combo = true;

        self
    }
}

/// Insert the max combo of the score's map
pub async fn prepare_score(ctx: &Context, score: &mut Score) -> OsuResult<()> {
    let mode = score.mode;

    let valid_score = score
        .map
        .as_mut()
        .filter(|_| matches!(mode, GameMode::STD | GameMode::CTB))
        .filter(|map| map.max_combo.is_none());

    if let Some(map) = valid_score {
        if let Ok(Some(combo)) = ctx.psql().get_beatmap_combo(map.map_id).await {
            map.max_combo = Some(combo);
        } else {
            let beatmap = ctx.osu().beatmap().map_id(map.map_id).await?;

            if let Err(err) = ctx.psql().insert_beatmap(&beatmap).await {
                warn!("{:?}", Report::new(err));
            }

            map.max_combo = beatmap.max_combo;
        }
    }

    Ok(())
}

/// Insert the max combos of the scores' maps
fn prepare_scores<'c, F>(
    ctx: &'c Context,
    fut: F,
) -> impl 'c + Future<Output = OsuResult<Vec<Score>>>
where
    F: 'c + Future<Output = OsuResult<Vec<Score>>>,
{
    fut.then(move |result| async move {
        let mut scores = result?;

        // Gather combos from DB
        let map_ids: Vec<_> = scores
            .iter()
            .filter_map(|s| s.map.as_ref())
            .filter(|map| map.max_combo.is_none() && map.mode != GameMode::MNA)
            .map(|map| map.map_id as i32)
            .collect();

        if map_ids.is_empty() {
            return Ok(scores);
        }

        let combos = match ctx.psql().get_beatmaps_combo(&map_ids).await {
            Ok(map) => map,
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to get map combos");
                warn!("{report:?}");

                HashMap::default()
            }
        };

        // Insert all combos from the database and collect remaining map ids
        let mut map_ids = Vec::with_capacity(map_ids.len() - combos.len());

        let map_ids_iter = scores
            .iter_mut()
            .filter_map(|score| score.map.as_mut())
            .filter(|map| map.max_combo.is_none() && map.mode != GameMode::MNA)
            .filter_map(|map| match combos.get(&map.map_id) {
                Some(Some(combo)) => {
                    map.max_combo = Some(*combo);

                    None
                }
                None | Some(None) => Some(map.map_id),
            });

        map_ids.extend(map_ids_iter);

        if map_ids.is_empty() {
            return Ok(scores);
        }

        // Request remaining maps and insert their combos
        for map in ctx.osu().beatmaps(map_ids).await? {
            if let Some(combo) = map.max_combo {
                let map_opt = scores
                    .iter_mut()
                    .filter_map(|s| s.map.as_mut())
                    .find(|m| m.map_id == map.map_id);

                if let Some(map) = map_opt {
                    map.max_combo = Some(combo);

                    if let Err(err) = ctx.psql().insert_beatmap(map).await {
                        let report = Report::new(err).wrap_err("failed to insert map into DB");
                        warn!("{report:?}");
                    }
                }
            }
        }

        Ok(scores)
    })
}

pub async fn require_link(ctx: &Context, orig: &CommandOrigin<'_>) -> BotResult<()> {
    let content = "Either specify an osu! username or link yourself to an osu! profile via `/link`";

    orig.error(ctx, content).await.map_err(Error::from)
}

async fn get_globals_count(
    ctx: &Context,
    user: &User,
    mode: GameMode,
) -> BotResult<BTreeMap<usize, Cow<'static, str>>> {
    let mut counts = BTreeMap::new();
    let mut params = OsuStatsParams::new(user.username.as_str()).mode(mode);
    let mut get_amount = true;

    for rank in [50, 25, 15, 8] {
        if !get_amount {
            counts.insert(rank, Cow::Borrowed("0"));

            continue;
        }

        params.max_rank = rank;
        let (_, count) = ctx.client().get_global_scores(&params).await?;
        counts.insert(rank, Cow::Owned(with_comma_int(count).to_string()));

        if count == 0 {
            get_amount = false;
        }
    }

    let firsts = if let Some(firsts) = user.scores_first_count {
        Cow::Owned(with_comma_int(firsts).to_string())
    } else if get_amount {
        params.max_rank = 1;
        let (_, count) = ctx.client().get_global_scores(&params).await?;

        Cow::Owned(with_comma_int(count).to_string())
    } else {
        Cow::Borrowed("0")
    };

    counts.insert(1, firsts);

    Ok(counts)
}

pub trait Number: AddAssign + Copy + Div<Output = Self> + PartialOrd {
    fn zero() -> Self;
    fn max() -> Self;
    fn min() -> Self;
    fn inc(&mut self);
}

#[rustfmt::skip]
impl Number for f32 {
    fn zero() -> Self { 0.0 }
    fn max() -> Self { f32::MAX }
    fn min() -> Self { f32::MIN }
    fn inc(&mut self) { *self += 1.0 }
}

#[rustfmt::skip]
impl Number for u32 {
    fn zero() -> Self { 0 }
    fn max() -> Self { u32::MAX }
    fn min() -> Self { u32::MIN }
    fn inc(&mut self) { *self += 1 }
}

pub struct MinMaxAvg<N> {
    min: N,
    max: N,
    sum: N,
    len: N,
}

impl<N: Number> MinMaxAvg<N> {
    fn new() -> Self {
        Self {
            min: N::max(),
            max: N::min(),
            sum: N::zero(),
            len: N::zero(),
        }
    }

    pub fn add(&mut self, n: N) {
        if self.min > n {
            self.min = n;
        }

        if self.max < n {
            self.max = n;
        }

        self.sum += n;
        self.len.inc();
    }

    pub fn avg(&self) -> N {
        self.sum / self.len
    }

    pub fn min(&self) -> N {
        self.min
    }

    pub fn max(&self) -> N {
        self.max
    }
}

impl From<MinMaxAvg<f32>> for MinMaxAvg<u32> {
    fn from(other: MinMaxAvg<f32>) -> Self {
        Self {
            min: other.min as u32,
            max: other.max as u32,
            sum: other.sum as u32,
            len: other.len as u32,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, CommandOption, CreateOption)]
pub enum ScoreOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "BPM", value = "bpm")]
    Bpm,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Length", value = "len")]
    Length,
    #[option(name = "Misses", value = "misses")]
    Misses,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Map ranked date", value = "ranked_date")]
    RankedDate,
    #[option(name = "Score", value = "score")]
    Score,
    #[option(name = "Stars", value = "stars")]
    Stars,
}

impl Default for ScoreOrder {
    fn default() -> Self {
        Self::Pp
    }
}

impl ScoreOrder {
    pub async fn apply<S: SortableScore>(self, ctx: &Context, scores: &mut [S]) {
        fn clock_rate(mods: GameMods) -> f32 {
            if mods.contains(GameMods::DoubleTime) {
                1.5
            } else if mods.contains(GameMods::HalfTime) {
                0.75
            } else {
                1.0
            }
        }

        match self {
            Self::Acc => {
                scores.sort_unstable_by(|a, b| {
                    b.acc().partial_cmp(&a.acc()).unwrap_or(Ordering::Equal)
                });
            }
            Self::Bpm => scores.sort_unstable_by(|a, b| {
                let a_bpm = a.bpm() * clock_rate(a.mods());
                let b_bpm = b.bpm() * clock_rate(b.mods());

                b_bpm.partial_cmp(&a_bpm).unwrap_or(Ordering::Equal)
            }),
            Self::Combo => scores.sort_unstable_by_key(|s| Reverse(s.max_combo())),
            Self::Date => scores.sort_unstable_by_key(|s| Reverse(s.created_at())),
            Self::Length => scores.sort_unstable_by(|a, b| {
                let a_len = a.seconds_drain() as f32 / clock_rate(a.mods());
                let b_len = b.seconds_drain() as f32 / clock_rate(b.mods());

                b_len.partial_cmp(&a_len).unwrap_or(Ordering::Equal)
            }),
            Self::Misses => scores.sort_unstable_by(|a, b| {
                b.n_misses().cmp(&a.n_misses()).then_with(|| {
                    let hits_a = a.total_hits_sort();
                    let hits_b = b.total_hits_sort();

                    let ratio_a = a.n_misses() as f32 / hits_a as f32;
                    let ratio_b = b.n_misses() as f32 / hits_b as f32;

                    ratio_b
                        .partial_cmp(&ratio_a)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| hits_b.cmp(&hits_a))
                })
            }),
            Self::Pp => scores
                .sort_unstable_by(|a, b| b.pp().partial_cmp(&a.pp()).unwrap_or(Ordering::Equal)),
            Self::RankedDate => {
                let mut mapsets = HashMap::new();
                let mut new_mapsets = HashMap::new();

                for score in scores.iter() {
                    let mapset_id = score.mapset_id();

                    match ctx.psql().get_beatmapset::<Beatmapset>(mapset_id).await {
                        Ok(Beatmapset {
                            ranked_date: Some(date),
                            ..
                        }) => {
                            mapsets.insert(mapset_id, date);
                        }
                        Ok(_) => {
                            warn!("Missing ranked date for top score DB mapset {mapset_id}");

                            continue;
                        }
                        Err(err) => {
                            let report = Report::new(err).wrap_err("failed to get mapset");
                            warn!("{report:?}");

                            match ctx.osu().beatmapset(mapset_id).await {
                                Ok(mapset) => {
                                    new_mapsets.insert(mapset_id, mapset);
                                }
                                Err(err) => {
                                    let report =
                                        Report::new(err).wrap_err("failed to request mapset");
                                    warn!("{report:?}");

                                    continue;
                                }
                            }
                        }
                    };
                }

                if !new_mapsets.is_empty() {
                    let result: Result<(), _> = new_mapsets
                        .values()
                        .map(|mapset| ctx.psql().insert_beatmapset(mapset).map_ok(|_| ()))
                        .collect::<FuturesUnordered<_>>()
                        .try_collect()
                        .await;

                    if let Err(err) = result {
                        let report = Report::new(err).wrap_err("failed to insert mapsets");
                        warn!("{report:?}");
                    } else {
                        info!("Inserted {} mapsets into the DB", new_mapsets.len());
                    }

                    let iter = new_mapsets
                        .into_iter()
                        .filter_map(|(id, mapset)| Some((id, mapset.ranked_date?)));

                    mapsets.extend(iter);
                }

                scores.sort_unstable_by(|a, b| {
                    let mapset_a = a.mapset_id();
                    let mapset_b = b.mapset_id();

                    let date_a = mapsets.get(&mapset_a).copied().unwrap_or_else(Utc::now);
                    let date_b = mapsets.get(&mapset_b).copied().unwrap_or_else(Utc::now);

                    date_a.cmp(&date_b)
                })
            }
            Self::Score => scores.sort_unstable_by_key(|score| Reverse(score.score())),
            Self::Stars => {
                let mut stars = HashMap::new();

                for score in scores.iter() {
                    let score_id = score.score_id();
                    let map_id = score.map_id();

                    if !score.mods().changes_stars(score.mode()) {
                        stars.insert(score_id, score.stars());

                        continue;
                    }

                    let stars_ = match PpCalculator::new(ctx, map_id).await {
                        Ok(mut calc) => calc.mods(score.mods()).stars() as f32,
                        Err(err) => {
                            warn!("{:?}", Report::new(err));

                            continue;
                        }
                    };

                    stars.insert(score_id, stars_);
                }

                scores.sort_unstable_by(|a, b| {
                    let stars_a = stars.get(&a.score_id()).unwrap_or(&0.0);
                    let stars_b = stars.get(&b.score_id()).unwrap_or(&0.0);

                    stars_b.partial_cmp(stars_a).unwrap_or(Ordering::Equal)
                })
            }
        }
    }
}

enum NameExtraction {
    Name(Username),
    Err(Error),
    Content(String),
    None,
}

// fn option_mode() -> MyCommandOption {
//     MyCommandOption::builder(MODE, SPECIFY_MODE).string(mode_choices(), false)
// }

// fn option_name() -> MyCommandOption {
//     MyCommandOption::builder(NAME, "Specify a username").string(Vec::new(), false)
// }

// fn option_discord() -> MyCommandOption {
//     let help = "Instead of specifying an osu! username with the `name` option, \
//         you can use this `discord` option to choose a discord user.\n\
//         For it to work, the user must be linked to an osu! account i.e. they must have used \
//         the `/link` or `/config` command to verify their account.";

//     MyCommandOption::builder(DISCORD, "Specify a linked discord user")
//         .help(help)
//         .user(false)
// }

// fn option_mods_explicit() -> MyCommandOption {
//     let description =
//         "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)";

//     let help = "Filter out all scores that don't match the specified mods.\n\
//         Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
//         or `-mods!` for excluded mods.\n\
//         Examples:\n\
//         - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
//         - `+hdhr!`: Scores must have exactly `HDHR`\n\
//         - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
//         - `-nm!`: Scores can not be nomod so there must be any other mod";

//     MyCommandOption::builder(MODS, description)
//         .help(help)
//         .string(Vec::new(), false)
// }

// fn option_country() -> MyCommandOption {
//     MyCommandOption::builder(COUNTRY, SPECIFY_COUNTRY).string(Vec::new(), false)
// }

// fn option_mods(filter: bool) -> MyCommandOption {
//     let help = if filter {
//         "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax, \
//         e.g. `hdhr` or `+hdhr!`, and filter out all scores that don't match those mods."
//     } else {
//         "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
//     };

//     MyCommandOption::builder(MODS, "Specify mods e.g. hdhr or nm")
//         .help(help)
//         .string(Vec::new(), false)
// }

// fn option_map() -> MyCommandOption {
//     let help = "Specify a map either by map url or map id.\n\
//         If none is specified, it will search in the recent channel history \
//         and pick the first map it can find.";

//     MyCommandOption::builder(MAP, "Specify a map url or map id")
//         .help(help)
//         .string(Vec::new(), false)
// }

// fn option_query() -> MyCommandOption {
//     let query_description = "Specify a search query containing artist, difficulty, AR, BPM, ...";

//     let query_help = "Filter out scores similarly as you filter maps in osu! itself.\n\
//         You can specify the artist, creator, difficulty, title, or limit values such as \
//         ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
//         While ar & co will be adjusted to mods, stars will not.";

//     MyCommandOption::builder("query", query_description)
//         .help(query_help)
//         .string(Vec::new(), false)
// }
