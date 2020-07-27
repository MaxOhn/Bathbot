mod common;
mod leaderboard;
mod link;
mod map;
mod match_costs;
mod most_played;
mod most_played_common;
mod nochoke;
mod osustats_globals;
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

// TODO: Remove pubs(?)
pub use common::*;
pub use leaderboard::*;
pub use link::*;
pub use map::*;
pub use match_costs::*;
pub use most_played::*;
pub use most_played_common::*;
pub use nochoke::*;
pub use osustats_globals::*;
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

use crate::{util::MessageExt, BotResult, Context};

use twilight::model::channel::Message;

async fn require_link(ctx: &Context, msg: &Message) -> BotResult<()> {
    let prefix = match msg.guild_id {
        Some(guild_id) => ctx.config_first_prefix(guild_id),
        None => String::from("<"),
    };
    let content = format!(
        "Either specify an osu name or link your discord \
        to an osu profile via `{}link osuname`",
        prefix
    );
    msg.respond(ctx, content).await
}
