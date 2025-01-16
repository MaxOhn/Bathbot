use bathbot_macros::SlashCommand;
use bathbot_model::command_fields::GameModeOption;
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::channel::Attachment;

pub use self::reshard::RESHARD_TX;
use self::{add_bg::*, cache::*, request_members::*};
use crate::{
    commands::owner::reshard::reshard,
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

mod add_bg;
mod cache;
mod request_members;
mod reshard;
mod tracking_stats;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "owner", desc = "You won't be able to use this :^)")]
#[flags(ONLY_OWNER, SKIP_DEFER)]
#[allow(clippy::large_enum_variant)]
pub enum Owner {
    #[command(name = "add_bg")]
    AddBg(OwnerAddBg),
    #[command(name = "cache")]
    Cache(OwnerCache),
    #[command(name = "requestmembers")]
    RequestMembers(OwnerRequestMembers),
    #[command(name = "reshard")]
    Reshard(OwnerReshard),
    #[command(name = "tracking")]
    Tracking(OwnerTracking),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "add_bg", desc = "Add a background to the bg game")]
pub struct OwnerAddBg {
    #[command(desc = "Add a png or jpg image with the mapset id as name")]
    image: Attachment,
    #[command(desc = "Specify the mode of the background's map")]
    mode: Option<GameModeOption>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "cache", desc = "Display stats about the internal cache")]
pub struct OwnerCache;

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "requestmembers",
    desc = "Manually queue a member request for a guild"
)]
pub struct OwnerRequestMembers {
    #[command(desc = "The guild id of which members should be requested")]
    guild_id: String, // u64 might be larger than what discord accepts as valid integer
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "reshard", desc = "Reshard the gateway")]
pub struct OwnerReshard;

#[derive(CommandModel, CreateCommand)]
#[command(name = "tracking", desc = "Stuff about osu!tracking")]
pub enum OwnerTracking {
    #[command(name = "stats")]
    Stats(OwnerTrackingStats),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "stats", desc = "Display tracking stats")]
pub struct OwnerTrackingStats;

async fn slash_owner(mut command: InteractionCommand) -> Result<()> {
    match Owner::from_interaction(command.input_data())? {
        Owner::AddBg(bg) => addbg(command, bg).await,
        Owner::Cache(_) => cache(command).await,
        Owner::RequestMembers(args) => request_members(command, &args.guild_id).await,
        Owner::Reshard(_) => reshard(command).await,
        Owner::Tracking(OwnerTracking::Stats(_)) => tracking_stats::trackingstats(command).await,
    }
}
