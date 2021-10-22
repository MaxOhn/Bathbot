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
        numbers::with_comma_uint,
        MessageExt,
    },
    BotResult, CommandData, Context, Error,
};

use deadpool_redis::redis::AsyncCommands;
use futures::{
    future::FutureExt,
    stream::{FuturesUnordered, StreamExt},
};
use rosu_v2::prelude::{GameMode, Grade, OsuError, OsuResult, Score, User};
use std::{
    borrow::Cow,
    cmp::PartialOrd,
    collections::BTreeMap,
    fmt::Write,
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

pub async fn request_user(ctx: &Context, name: &str, mode: GameMode) -> OsuResult<User> {
    let key = format!("__{}_{}", name, mode as u8);

    let mut conn = match ctx.clients.redis.get().await {
        Ok(mut conn) => {
            if let Ok(bytes) = conn.get::<_, Vec<u8>>(&key).await {
                if !bytes.is_empty() {
                    ctx.stats.inc_cached_user();
                    let user =
                        serde_cbor::from_slice(&bytes).expect("failed to deserialize redis user");
                    debug!("Found user `{}` in cache", name);

                    return Ok(user);
                }
            }

            conn
        }
        Err(why) => {
            unwind_error!(warn, why, "Failed to get redis connection for user: {}");

            return ctx.osu().user(name).mode(mode).await;
        }
    };

    let mut user = ctx.osu().user(name).mode(mode).await?;

    // Remove html user page to reduce overhead
    user.page.take();

    let bytes = serde_cbor::to_vec(&user).expect("failed to serialize user");
    let set_fut = conn.set_ex::<_, _, ()>(key, bytes, USER_CACHE_SECONDS);

    // Cache users for 10 minutes
    if let Err(why) = set_fut.await {
        unwind_error!(debug, why, "Failed to insert bytes into cache: {}");
    }

    Ok(user)
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
            map.max_combo.replace(combo);
        } else {
            let beatmap = ctx.osu().beatmap().map_id(map.map_id).await?;

            if let Err(why) = ctx.psql().insert_beatmap(&beatmap).await {
                unwind_error!(warn, why, "Failed to insert beatmap: {}");
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
) -> impl 'c + Future<Output = Result<Vec<Score>, ErrorType>>
where
    F: 'c + Future<Output = OsuResult<Vec<Score>>>,
{
    fut.then(move |result| async move {
        let mut scores = result?;

        // If there's no score or its mania scores, return early
        let invalid_scores = scores
            .first()
            .filter(|s| s.map.is_some() && matches!(s.mode, GameMode::STD | GameMode::CTB))
            .is_none();

        if invalid_scores {
            return Ok(scores);
        }

        let map_ids: Vec<_> = scores
            .iter()
            .filter_map(|s| s.map.as_ref())
            .filter(|map| map.max_combo.is_none())
            .map(|map| map.map_id as i32)
            .collect();

        let combos = ctx.psql().get_beatmaps_combo(&map_ids).await?;

        let mut iter = scores
            .iter_mut()
            .map(|score| (combos.get(&score.map.as_ref().unwrap().map_id), score))
            .map(|(entry, score)| async move {
                let score_map = score.map.as_mut().unwrap();

                match entry {
                    Some(Some(combo)) => {
                        score_map.max_combo.replace(*combo);
                    }
                    None | Some(None) => {
                        let map = ctx.osu().beatmap().map_id(score_map.map_id).await?;

                        if let Err(why) = ctx.psql().insert_beatmap(&map).await {
                            unwind_error!(warn, why, "Failed to insert beatmap: {}");
                        }

                        score_map.max_combo = map.max_combo;
                    }
                }

                Ok::<_, Error>(())
            })
            .collect::<FuturesUnordered<_>>();

        while iter.next().await.transpose()?.is_some() {}

        drop(iter);

        Ok(scores)
    })
}

async fn require_link(ctx: &Context, data: &CommandData<'_>) -> BotResult<()> {
    // TODO: Remove temporary message again
    // let content = "Either specify an osu! username or link yourself to an osu! profile via `/link`";
    let content = "Due to a recent authorization update all links to osu! profiles were undone.\n\
        Use the `/link` command to link yourself to an osu! profile.";

    data.error(ctx, content).await
}

/// Be sure the whitespaces in the given name are __not__ replaced
async fn get_globals_count(
    ctx: &Context,
    user: &User,
    mode: GameMode,
) -> BotResult<BTreeMap<usize, Cow<'static, str>>> {
    let mut counts = BTreeMap::new();
    let mut params = OsuStatsParams::new(user.username.as_str()).mode(mode);
    let mut get_amount = true;

    for &rank in [50, 25, 15, 8].iter() {
        if !get_amount {
            counts.insert(rank, Cow::Borrowed("0"));

            continue;
        }

        params.rank_max = rank;
        let (_, count) = ctx.clients.custom.get_global_scores(&params).await?;
        counts.insert(rank, Cow::Owned(with_comma_uint(count).to_string()));

        if count == 0 {
            get_amount = false;
        }
    }

    let firsts = if let Some(firsts) = user.scores_first_count {
        Cow::Owned(with_comma_uint(firsts).to_string())
    } else if get_amount {
        params.rank_max = 1;
        let (_, count) = ctx.clients.custom.get_global_scores(&params).await?;

        Cow::Owned(with_comma_uint(count).to_string())
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
