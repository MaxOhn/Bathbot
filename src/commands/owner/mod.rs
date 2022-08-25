use std::sync::Arc;

use command_macros::SlashCommand;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::channel::Attachment;

use crate::{
    tracking::default_tracking_interval,
    util::{builder::MessageBuilder, interaction::InteractionCommand, InteractionCommandExt},
    BotResult, Context,
};

use self::{add_bg::*, add_country::*, cache::*, tracking_interval::*, tracking_stats::*};

use super::GameModeOption;

mod add_bg;
mod add_country;
mod cache;
mod tracking_interval;
mod tracking_stats;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "owner")]
#[flags(ONLY_OWNER, SKIP_DEFER)]
/// You won't be able to use this :^)
pub enum Owner {
    #[command(name = "add_bg")]
    AddBg(OwnerAddBg),
    #[command(name = "add_country")]
    AddCountry(OwnerAddCountry),
    #[command(name = "cache")]
    Cache(OwnerCache),
    #[command(name = "tracking")]
    Tracking(OwnerTracking),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "add_bg")]
/// Add a background the bg game
pub struct OwnerAddBg {
    /// Add a png or jpg image with the mapset id as name
    image: Attachment,
    /// Specify the mode of the background's map
    mode: Option<GameModeOption>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "add_country")]
/// Add a country for snipe commands
pub struct OwnerAddCountry {
    /// Specify the country code
    code: String,
    /// Specify the country name
    name: String,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "cache")]
/// Display stats about the internal cache
pub struct OwnerCache;

#[derive(CommandModel, CreateCommand)]
#[command(name = "tracking")]
/// Stuff about osu!tracking
pub enum OwnerTracking {
    #[command(name = "interval")]
    Interval(OwnerTrackingInterval),
    #[command(name = "stats")]
    Stats(OwnerTrackingStats),
    #[command(name = "toggle")]
    Toggle(OwnerTrackingToggle),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "interval")]
/// Adjust the tracking interval
pub struct OwnerTrackingInterval {
    /// Specify the interval in seconds, defaults to 9000
    number: Option<i64>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "stats")]
/// Display tracking stats
pub struct OwnerTrackingStats;

#[derive(CommandModel, CreateCommand)]
#[command(name = "toggle")]
/// Enable or disable tracking
pub struct OwnerTrackingToggle;

async fn slash_owner(ctx: Arc<Context>, mut command: InteractionCommand) -> BotResult<()> {
    match Owner::from_interaction(command.input_data())? {
        Owner::AddBg(bg) => addbg(ctx, command, bg).await,
        Owner::AddCountry(country) => addcountry(ctx, command, country).await,
        Owner::Cache(_) => cache(ctx, command).await,
        Owner::Tracking(OwnerTracking::Interval(interval)) => {
            let secs = interval
                .number
                .unwrap_or_else(|| default_tracking_interval().whole_seconds());

            trackinginterval(ctx, command, secs).await
        }
        Owner::Tracking(OwnerTracking::Stats(_)) => trackingstats(ctx, command).await,
        Owner::Tracking(OwnerTracking::Toggle(_)) => {
            ctx.tracking().toggle_tracking();
            let current = ctx.tracking().stop_tracking();
            let content = format!("Tracking toggle: {current} -> {}", !current);
            let builder = MessageBuilder::new().embed(content);
            command.callback(&ctx, builder, false).await?;

            Ok(())
        }
    }
}
