use std::sync::{atomic::Ordering, Arc};

use command_macros::SlashCommand;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{application::interaction::ApplicationCommand, channel::Attachment};

use crate::{
    tracking::{OSU_TRACKING_COOLDOWN, OSU_TRACKING_INTERVAL},
    util::{builder::MessageBuilder, ApplicationCommandExt},
    BotResult, Context,
};

use self::{
    add_bg::*, add_country::*, cache::*, tracking_cooldown::*, tracking_interval::*,
    tracking_stats::*,
};

use super::GameModeOption;

mod add_bg;
mod add_country;
mod cache;
mod tracking_cooldown;
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
    #[command(name = "cooldown")]
    Cooldown(OwnerTrackingCooldown),
    #[command(name = "interval")]
    Interval(OwnerTrackingInterval),
    #[command(name = "stats")]
    Stats(OwnerTrackingStats),
    #[command(name = "toggle")]
    Toggle(OwnerTrackingToggle),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "cooldown")]
/// Adjust the tracking cooldown
pub struct OwnerTrackingCooldown {
    /// Specify the cooldown in milliseconds, defaults to 5000.0
    number: Option<f64>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "interval")]
/// Adjust the tracking interval
pub struct OwnerTrackingInterval {
    /// Specify the interval in seconds, defaults to 7200
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

async fn slash_owner(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    match Owner::from_interaction(command.input_data())? {
        Owner::AddBg(bg) => addbg(ctx, command, bg).await,
        Owner::AddCountry(country) => addcountry(ctx, command, country).await,
        Owner::Cache(_) => cache(ctx, command).await,
        Owner::Tracking(OwnerTracking::Cooldown(cooldown)) => {
            let ms = cooldown.number.map_or(OSU_TRACKING_COOLDOWN, |n| n as f32);

            trackingcooldown(ctx, command, ms).await
        }
        Owner::Tracking(OwnerTracking::Interval(interval)) => {
            let secs = interval
                .number
                .unwrap_or(OSU_TRACKING_INTERVAL.num_seconds());

            trackinginterval(ctx, command, secs).await
        }
        Owner::Tracking(OwnerTracking::Stats(_)) => trackingstats(ctx, command).await,
        Owner::Tracking(OwnerTracking::Toggle(_)) => {
            ctx.tracking()
                .stop_tracking
                .fetch_nand(true, Ordering::SeqCst);

            let current = ctx.tracking().stop_tracking.load(Ordering::Acquire);
            let content = format!("Tracking toggle: {current} -> {}", !current);
            let builder = MessageBuilder::new().embed(content);
            command.callback(&ctx, builder, false).await?;

            Ok(())
        }
    }
}
