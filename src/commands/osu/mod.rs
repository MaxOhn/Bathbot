macro_rules! map_id {
    ($score:ident) => {
        $score.map.as_ref().map(|map| map.map_id)
    };
}

// mod avatar;
// mod bws;
// mod common;
// mod compare;
// mod country_snipe_list;
// mod country_snipe_stats;
// mod fix_score;
// mod leaderboard;
mod link;
// mod map;
// mod map_search;
// mod mapper;
mod match_costs;
// mod match_live;
mod medals;
// mod most_played;
// mod most_played_common;
// mod nochoke;
// mod osustats_counts;
// mod osustats_globals;
// mod osustats_list;
// mod player_snipe_list;
// mod player_snipe_stats;
// mod pp;
// mod profile;
// mod profile_compare;
// mod rank;
// mod rank_score;
// mod ranking;
// mod ranking_countries;
mod ratios;
// mod rebalance;
mod recent;
// mod simulate;
// mod sniped;
// mod sniped_difference;
// mod top;
// mod top_if;
// mod top_old;
// mod whatif;

// pub use avatar::*;
// pub use bws::*;
// pub use common::*;
// pub use compare::*;
// pub use country_snipe_list::*;
// pub use country_snipe_stats::*;
// pub use fix_score::*;
// pub use leaderboard::*;
pub use link::*;
// pub use map::*;
// pub use map_search::*;
// pub use mapper::*;
pub use match_costs::*;
// pub use match_live::*;
pub use medals::*;
// pub use most_played::*;
// pub use most_played_common::*;
// pub use nochoke::*;
// pub use osustats_counts::*;
// pub use osustats_globals::*;
// pub use osustats_list::*;
// pub use player_snipe_list::*;
// pub use player_snipe_stats::*;
// pub use pp::*;
// pub use profile::*;
// pub use profile_compare::*;
// pub use rank::*;
// pub use rank_score::*;
// pub use ranking::*;
// pub use ranking_countries::*;
pub use ratios::*;
// pub use rebalance::*;
pub use recent::*;
// pub use simulate::*;
// pub use sniped::*;
// pub use sniped_difference::*;
// pub use top::*;
// pub use top_if::*;
// pub use top_old::*;
// pub use whatif::*;

use crate::{
    custom_client::OsuStatsParams,
    util::{numbers::with_comma_uint, MessageExt},
    BotResult, CommandData, Context, Error,
};

use deadpool_redis::redis::AsyncCommands;
use futures::{
    future::FutureExt,
    stream::{FuturesUnordered, StreamExt},
};
use rosu_v2::prelude::{GameMode, OsuError, OsuResult, Score, User};
use std::{
    borrow::Cow,
    cmp::PartialOrd,
    collections::BTreeMap,
    fmt::Write,
    future::Future,
    ops::{AddAssign, Div},
};
use twilight_model::{
    application::{command::CommandOptionChoice, interaction::ApplicationCommand},
    channel::Message,
};

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

async fn request_user(ctx: &Context, name: &str, mode: Option<GameMode>) -> OsuResult<User> {
    let mut key = String::with_capacity(2 + name.len() + 2 * mode.is_some() as usize);
    let _ = write!(key, "__{}", name);

    if let Some(mode) = mode {
        let _ = write!(key, "_{}", mode as u8);
    }

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
            let user_fut = ctx.osu().user(name);

            return match mode {
                Some(mode) => user_fut.mode(mode).await,
                None => user_fut.await,
            };
        }
    };

    let user_fut = ctx.osu().user(name);

    let mut user = match mode {
        Some(mode) => user_fut.mode(mode).await?,
        None => user_fut.await?,
    };

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
    match data {
        CommandData::Message { msg, .. } => require_link_msg(ctx, msg).await,
        CommandData::Interaction { command } => require_link_slash(ctx, command).await,
    }
}

async fn require_link_msg(ctx: &Context, msg: &Message) -> BotResult<()> {
    let prefix = ctx.config_first_prefix(msg.guild_id);

    let content = format!(
        "Either specify an osu name or link your discord \
        to an osu profile via `{}link \"osu! username\"`",
        prefix
    );

    msg.error(ctx, content).await
}

async fn require_link_slash(ctx: &Context, command: &ApplicationCommand) -> BotResult<()> {
    let content = "Either specify an osu name or link your discord \
    to an osu profile with the `/link` command";

    command.error(&ctx, content).await
}

/// Be sure the whitespaces in the given name are __not__ replaced
async fn get_globals_count(
    ctx: &Context,
    name: &str,
    mode: GameMode,
) -> BotResult<BTreeMap<usize, Cow<'static, str>>> {
    let mut counts = BTreeMap::new();
    let mut params = OsuStatsParams::new(name).mode(mode);
    let mut get_amount = true;

    for &rank in [50, 25, 15, 8, 1].iter() {
        if !get_amount {
            counts.insert(rank, Cow::Borrowed("0"));

            continue;
        }

        params = params.rank_max(rank);
        let (_, count) = ctx.clients.custom.get_global_scores(&params).await?;
        counts.insert(rank, Cow::Owned(with_comma_uint(count).to_string()));

        if count == 0 {
            get_amount = false;
        }
    }

    Ok(counts)
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
            name: "osu".to_owned(),
            value: "osu".to_owned(),
        },
        CommandOptionChoice::String {
            name: "taiko".to_owned(),
            value: "taiko".to_owned(),
        },
        CommandOptionChoice::String {
            name: "catch".to_owned(),
            value: "catch".to_owned(),
        },
        CommandOptionChoice::String {
            name: "mania".to_owned(),
            value: "mania".to_owned(),
        },
    ]
}
