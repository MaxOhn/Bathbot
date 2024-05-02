use bathbot_macros::SlashCommand;
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::channel::Attachment;

pub use self::reshard::RESHARD_TX;
use self::{add_bg::*, cache::*, request_members::*};
#[cfg(feature = "osutracking")]
use self::{tracking_interval::*, tracking_stats::*};
use super::GameModeOption;
#[cfg(feature = "osutracking")]
use crate::tracking::default_tracking_interval;
use crate::{
    commands::owner::reshard::reshard,
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

mod add_bg;
mod cache;
mod request_members;
mod reshard;

#[cfg(feature = "osutracking")]
mod tracking_interval;

#[cfg(feature = "osutracking")]
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
    #[cfg(feature = "osutracking")]
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

#[cfg(feature = "osutracking")]
#[derive(CommandModel, CreateCommand)]
#[command(name = "tracking", desc = "Stuff about osu!tracking")]
pub enum OwnerTracking {
    #[command(name = "interval")]
    Interval(OwnerTrackingInterval),
    #[command(name = "stats")]
    Stats(OwnerTrackingStats),
    #[command(name = "toggle")]
    Toggle(OwnerTrackingToggle),
}

#[cfg(feature = "osutracking")]
#[derive(CommandModel, CreateCommand)]
#[command(name = "interval", desc = "Adjust the tracking interval")]
pub struct OwnerTrackingInterval {
    #[command(desc = "Specify the interval in seconds, defaults to 9000")]
    number: Option<i64>,
}

#[cfg(feature = "osutracking")]
#[derive(CommandModel, CreateCommand)]
#[command(name = "stats", desc = "Display tracking stats")]
pub struct OwnerTrackingStats;

#[cfg(feature = "osutracking")]
#[derive(CommandModel, CreateCommand)]
#[command(name = "toggle", desc = "Enable or disable tracking")]
pub struct OwnerTrackingToggle;

async fn slash_owner(mut command: InteractionCommand) -> Result<()> {
    match Owner::from_interaction(command.input_data())? {
        Owner::AddBg(bg) => addbg(command, bg).await,
        Owner::Cache(_) => cache(command).await,
        Owner::RequestMembers(args) => request_members(command, &args.guild_id).await,
        Owner::Reshard(_) => reshard(command).await,
        #[cfg(feature = "osutracking")]
        Owner::Tracking(OwnerTracking::Interval(interval)) => {
            let secs = interval
                .number
                .unwrap_or_else(|| default_tracking_interval().whole_seconds());

            trackinginterval(command, secs).await
        }
        #[cfg(feature = "osutracking")]
        Owner::Tracking(OwnerTracking::Stats(_)) => trackingstats(command).await,
        #[cfg(feature = "osutracking")]
        Owner::Tracking(OwnerTracking::Toggle(_)) => {
            let tracking = crate::core::Context::tracking();
            tracking.toggle_tracking();
            let current = tracking.stop_tracking();
            let content = format!("Tracking toggle: {current} -> {}", !current);
            let builder = bathbot_util::MessageBuilder::new().embed(content);
            command.callback(builder, false).await?;

            Ok(())
        }
    }
}
