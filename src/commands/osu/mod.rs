macro_rules! map_id {
    ($score:ident) => {
        $score.map.as_ref().map(|map| map.map_id)
    };
}

mod avatar;
mod bws;
mod compare;
mod fix;
mod leaderboard;
mod link;
mod map;
mod map_search;
mod match_costs;
mod match_live;
mod medals;
mod most_played;
mod osekai;
mod osustats;
mod profile;
mod ranking;
mod ratios;
mod reach;
mod recent;
mod simulate;
mod snipe;
mod top;
mod whatif;

pub use avatar::*;
pub use bws::*;
pub use compare::*;
pub use fix::*;
use hashbrown::HashMap;
pub use leaderboard::*;
pub use link::*;
pub use map::*;
pub use map_search::*;
pub use match_costs::*;
pub use match_live::*;
pub use medals::*;
pub use most_played::*;
pub use osekai::*;
pub use osustats::*;
pub use profile::*;
pub use ranking::*;
pub use ratios::*;
pub use reach::*;
pub use recent::*;
pub use simulate::*;
pub use snipe::*;
pub use top::*;
pub use whatif::*;

use crate::{
    custom_client::OsuStatsParams,
    util::{
        constants::common_literals::{
            COUNTRY, CTB, DISCORD, MANIA, MAP, MODE, MODS, NAME, OSU, SPECIFY_COUNTRY,
            SPECIFY_MODE, TAIKO,
        },
        numbers::with_comma_int,
        CowUtils, MessageExt,
    },
    BotResult, CommandData, Context, Error,
};

use deadpool_redis::redis::AsyncCommands;
use eyre::Report;
use futures::future::FutureExt;
use rosu_v2::{
    prelude::{BeatmapUserScore, GameMode, GameMods, Grade, OsuError, OsuResult, Score, User},
    request::GetUserScores,
    Osu,
};
use std::{
    borrow::Cow,
    cmp::PartialOrd,
    collections::BTreeMap,
    future::Future,
    ops::{AddAssign, Div},
};
use twilight_model::application::command::CommandOptionChoice;

use super::MyCommandOption;

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

const USER_CACHE_SECONDS: usize = 600;

async fn get_user(ctx: &Context, user: &UserArgs<'_>) -> OsuResult<User> {
    if let Some(alt_name) = user.whitespaced_name() {
        match get_user_cached(ctx, user).await {
            Err(OsuError::NotFound) => {
                let user = UserArgs::new(&alt_name, user.mode);

                get_user_cached(ctx, &user).await
            }
            result => result,
        }
    } else {
        get_user_cached(ctx, user).await
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

async fn get_user_cached(ctx: &Context, user: &UserArgs<'_>) -> OsuResult<User> {
    let key = format!("__{}_{}", user.name, user.mode as u8);

    let mut conn = match ctx.clients.redis.get().await {
        Ok(mut conn) => {
            if let Ok(bytes) = conn.get::<_, Vec<u8>>(&key).await {
                if !bytes.is_empty() {
                    ctx.stats.inc_cached_user();
                    trace!("Found user `{}` in cache", user.name);
                    let user =
                        serde_cbor::from_slice(&bytes).expect("failed to deserialize redis user");

                    return Ok(user);
                }
            }

            conn
        }
        Err(why) => {
            let report = Report::new(why).wrap_err("failed to get redis connection");
            warn!("{:?}", report);

            return ctx.osu().user(user.name).mode(user.mode).await;
        }
    };

    let mut user = ctx.osu().user(user.name).mode(user.mode).await?;

    // Remove html user page to reduce overhead
    user.page.take();

    let bytes = serde_cbor::to_vec(&user).expect("failed to serialize user");

    // Cache users for 10 minutes and update username in DB
    let set_fut = conn.set_ex::<_, _, ()>(key, bytes, USER_CACHE_SECONDS);
    let name_update_fut = ctx.psql().upsert_osu_name(user.user_id, &user.username);

    let (set_result, name_update_result) = tokio::join!(set_fut, name_update_fut);

    if let Err(why) = set_result {
        let report = Report::new(why).wrap_err("failed to insert bytes into cache");
        warn!("{:?}", report);
    }

    if let Err(why) = name_update_result {
        let report = Report::new(why).wrap_err("failed to update osu! username");
        warn!("{:?}", report);
    }

    Ok(user)
}

async fn get_user_and_scores<'c>(
    ctx: &'c Context,
    mut user: UserArgs<'_>,
    scores: &ScoreArgs<'c>,
) -> OsuResult<(User, Vec<Score>)> {
    if let Some(alt_name) = user.whitespaced_name() {
        match get_user_cached(ctx, &user).await {
            Ok(u) => Ok((u, get_scores(ctx, &user, scores).await?)),
            Err(OsuError::NotFound) => {
                user.name = &alt_name;

                let user_fut = get_user_cached(ctx, &user);
                let scores_fut = get_scores(ctx, &user, scores);

                tokio::try_join!(user_fut, scores_fut)
            }
            Err(err) => Err(err),
        }
    } else {
        let user_fut = get_user_cached(ctx, &user);
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

    if scores.with_combo {
        prepare_scores(ctx, scores_fut).await
    } else {
        scores_fut.await
    }
}

struct UserArgs<'n> {
    name: &'n str,
    mode: GameMode,
}

impl<'n> UserArgs<'n> {
    fn new(name: &'n str, mode: GameMode) -> Self {
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
                warn!("{:?}", report);

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
                if let Some(map) = scores
                    .iter_mut()
                    .filter_map(|s| s.map.as_mut())
                    .find(|m| m.map_id == map.map_id)
                {
                    map.max_combo = Some(combo);

                    if let Err(err) = ctx.psql().insert_beatmap(map).await {
                        let report = Report::new(err).wrap_err("failed to insert map into DB");
                        warn!("{:?}", report);
                    }
                }
            }
        }

        Ok(scores)
    })
}

async fn require_link(ctx: &Context, data: &CommandData<'_>) -> BotResult<()> {
    let content = "Either specify an osu! username or link yourself to an osu! profile via `/link`";

    data.error(ctx, content).await
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

        params.rank_max = rank;
        let (_, count) = ctx.clients.custom.get_global_scores(&params).await?;
        counts.insert(rank, Cow::Owned(with_comma_int(count).to_string()));

        if count == 0 {
            get_amount = false;
        }
    }

    let firsts = if let Some(firsts) = user.scores_first_count {
        Cow::Owned(with_comma_int(firsts).to_string())
    } else if get_amount {
        params.rank_max = 1;
        let (_, count) = ctx.clients.custom.get_global_scores(&params).await?;

        Cow::Owned(with_comma_int(count).to_string())
    } else {
        Cow::Borrowed("0")
    };

    counts.insert(1, firsts);

    Ok(counts)
}

#[derive(Copy, Clone)]
pub enum GradeArg {
    Single(Grade),
    Range { bot: Grade, top: Grade },
}

impl GradeArg {
    pub fn include_fails(&self) -> bool {
        matches!(self,
            Self::Single(g)
                | Self::Range { bot: g, .. }
                | Self::Range { top: g, ..} if *g == Grade::F
        )
    }
}

pub trait MinMaxAvgBasic {
    type Value: PartialOrd + AddAssign + Inc + Div<Output = Self::Value> + Copy;

    // Implement these
    fn new() -> Self;

    fn get(&self) -> (Self::Value, Self::Value, Self::Value, Self::Value);

    fn get_mut(
        &mut self,
    ) -> (
        &mut Self::Value,
        &mut Self::Value,
        &mut Self::Value,
        &mut Self::Value,
    );

    // Don't implement these
    fn add(&mut self, value: Self::Value) {
        let (min, max, sum, len) = self.get_mut();

        if *min > value {
            *min = value;
        }

        if *max < value {
            *max = value;
        }

        *sum += value;
        len.inc();
    }

    fn min(&self) -> Self::Value {
        let (min, _, _, _) = self.get();

        min
    }

    fn max(&self) -> Self::Value {
        let (_, max, _, _) = self.get();

        max
    }

    fn avg(&self) -> Self::Value {
        let (_, _, sum, len) = self.get();

        sum / len
    }
}

pub struct MinMaxAvgU32 {
    min: u32,
    max: u32,
    sum: u32,
    len: u32,
}

impl MinMaxAvgBasic for MinMaxAvgU32 {
    type Value = u32;

    fn new() -> Self {
        Self {
            min: u32::MAX,
            max: 0,
            sum: 0,
            len: 0,
        }
    }

    fn get(&self) -> (u32, u32, u32, u32) {
        (self.min, self.max, self.sum, self.len)
    }

    fn get_mut(&mut self) -> (&mut u32, &mut u32, &mut u32, &mut u32) {
        (&mut self.min, &mut self.max, &mut self.sum, &mut self.len)
    }
}

impl From<MinMaxAvgF32> for MinMaxAvgU32 {
    fn from(val: MinMaxAvgF32) -> Self {
        Self {
            min: val.min as u32,
            max: val.max as u32,
            sum: val.sum as u32,
            len: val.len as u32,
        }
    }
}

pub struct MinMaxAvgF32 {
    min: f32,
    max: f32,
    sum: f32,
    len: f32,
}

impl MinMaxAvgBasic for MinMaxAvgF32 {
    type Value = f32;

    fn new() -> Self {
        Self {
            min: f32::MAX,
            max: 0.0,
            sum: 0.0,
            len: 0.0,
        }
    }

    fn get(&self) -> (f32, f32, f32, f32) {
        (self.min, self.max, self.sum, self.len)
    }

    fn get_mut(&mut self) -> (&mut f32, &mut f32, &mut f32, &mut f32) {
        (&mut self.min, &mut self.max, &mut self.sum, &mut self.len)
    }
}

pub trait Inc {
    fn inc(&mut self);
}

impl Inc for f32 {
    fn inc(&mut self) {
        *self += 1.0;
    }
}

impl Inc for u32 {
    fn inc(&mut self) {
        *self += 1;
    }
}

fn mode_choices() -> Vec<CommandOptionChoice> {
    vec![
        CommandOptionChoice::String {
            name: OSU.to_owned(),
            value: OSU.to_owned(),
        },
        CommandOptionChoice::String {
            name: TAIKO.to_owned(),
            value: TAIKO.to_owned(),
        },
        CommandOptionChoice::String {
            name: CTB.to_owned(),
            value: CTB.to_owned(),
        },
        CommandOptionChoice::String {
            name: MANIA.to_owned(),
            value: MANIA.to_owned(),
        },
    ]
}

fn option_mode() -> MyCommandOption {
    MyCommandOption::builder(MODE, SPECIFY_MODE).string(mode_choices(), false)
}

fn option_name() -> MyCommandOption {
    MyCommandOption::builder(NAME, "Specify a username").string(Vec::new(), false)
}

fn option_discord() -> MyCommandOption {
    let help = "Instead of specifying an osu! username with the `name` option, \
        you can use this `discord` option to choose a discord user.\n\
        For it to work, the user must be linked to an osu! account i.e. they must have used \
        the `/link` or `/config` command to verify their account.";

    MyCommandOption::builder(DISCORD, "Specify a linked discord user")
        .help(help)
        .user(false)
}

fn option_mods_explicit() -> MyCommandOption {
    let description =
        "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)";

    let help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
        or `-mods!` for excluded mods.\n\
        Examples:\n\
        - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
        - `+hdhr!`: Scores must have exactly `HDHR`\n\
        - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
        - `-nm!`: Scores can not be nomod so there must be any other mod";

    MyCommandOption::builder(MODS, description)
        .help(help)
        .string(Vec::new(), false)
}

fn option_country() -> MyCommandOption {
    MyCommandOption::builder(COUNTRY, SPECIFY_COUNTRY).string(Vec::new(), false)
}

fn option_mods(filter: bool) -> MyCommandOption {
    let help = if filter {
        "Specify mods either directly or through the explicit `+_!` / `+_` syntax, \
        e.g. `hdhr` or `+hdhr!`, and filter out all scores that don't match those mods."
    } else {
        "Specify mods either directly or through the explicit `+_!` / `+_` syntax e.g. `hdhr` or `+hdhr!`"
    };

    MyCommandOption::builder(MODS, "Specify mods e.g. hdhr or nm")
        .help(help)
        .string(Vec::new(), false)
}

fn option_map() -> MyCommandOption {
    let help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find.";

    MyCommandOption::builder(MAP, "Specify a map url or map id")
        .help(help)
        .string(Vec::new(), false)
}
