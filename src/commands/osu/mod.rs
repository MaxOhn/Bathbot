mod avatar;
mod common;
mod country_snipe_list;
mod country_snipe_stats;
mod leaderboard;
mod link;
mod map;
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
pub use common::*;
pub use country_snipe_list::*;
pub use country_snipe_stats::*;
pub use leaderboard::*;
pub use link::*;
pub use map::*;
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

use rosu::models::GameMode;
use std::collections::BTreeMap;
use twilight::model::channel::Message;

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
) -> BotResult<BTreeMap<usize, String>> {
    let mut counts = BTreeMap::new();
    let mut params = OsuStatsParams::new(name).mode(mode);
    let mut get_amount = true;
    for rank in [50, 25, 15, 8, 1].iter() {
        if !get_amount {
            counts.insert(*rank, "0".to_owned());
            continue;
        }
        params = params.rank_max(*rank);
        let (_, count) = ctx.clients.custom.get_global_scores(&params).await?;
        counts.insert(*rank, numbers::with_comma_int(count as u64));
        if count == 0 {
            get_amount = false;
        }
    }
    Ok(counts)
}
