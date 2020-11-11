mod avatar;
mod bws;
mod common;
mod country_snipe_list;
mod country_snipe_stats;
mod leaderboard;
mod link;
mod map;
mod mapper;
mod match_costs;
mod most_played;
mod most_played_common;
mod nochoke;
mod osustats_counts;
mod osustats_globals;
mod osustats_list;
mod player_snipe_stats;
mod pp;
mod profile;
mod profile_compare;
mod rank;
mod ratios;
mod recent;
mod recent_lb;
mod scores;
mod simulate;
mod simulate_recent;
mod top;
mod whatif;

pub use avatar::*;
pub use bws::*;
pub use common::*;
pub use country_snipe_list::*;
pub use country_snipe_stats::*;
pub use leaderboard::*;
pub use link::*;
pub use map::*;
pub use mapper::*;
pub use match_costs::*;
pub use most_played::*;
pub use most_played_common::*;
pub use nochoke::*;
pub use osustats_counts::*;
pub use osustats_globals::*;
pub use osustats_list::*;
pub use player_snipe_stats::*;
pub use pp::*;
pub use profile::*;
pub use profile_compare::*;
pub use rank::*;
pub use ratios::*;
pub use recent::*;
pub use recent_lb::*;
pub use scores::*;
pub use simulate::*;
pub use simulate_recent::*;
pub use top::*;
pub use whatif::*;

use crate::{
    custom_client::OsuStatsParams,
    util::{numbers, MessageExt},
    BotResult, Context,
};

use rosu::model::GameMode;
use std::{
    borrow::Cow,
    cmp::PartialOrd,
    collections::BTreeMap,
    ops::{AddAssign, Div},
};
use twilight_model::channel::Message;

async fn require_link(ctx: &Context, msg: &Message) -> BotResult<()> {
    let prefix = ctx.config_first_prefix(msg.guild_id);
    let content = format!(
        "Either specify an osu name or link your discord \
        to an osu profile via `{}link osuname`",
        prefix
    );
    msg.error(ctx, content).await
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
    for rank in [50, 25, 15, 8, 1].iter() {
        if !get_amount {
            counts.insert(*rank, Cow::Borrowed("0"));
            continue;
        }
        params = params.rank_max(*rank);
        let (_, count) = ctx.clients.custom.get_global_scores(&params).await?;
        counts.insert(*rank, Cow::Owned(numbers::with_comma_u64(count as u64)));
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
