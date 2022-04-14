use std::sync::Arc;

use command_macros::SlashCommand;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{util::ApplicationCommandExt, BotResult, Context};

pub use self::{addstream::*, removestream::*, tracked::*};

pub mod addstream;
pub mod removestream;
pub mod tracked;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "trackstream",
    help = "Track a twitch stream in this channel.\n\
    When the stream goes online, a notification will be send to this channel within a few minutes."
)]
#[flags(AUTHORITY, ONLY_GUILDS)]
/// Track a twitch stream or list all tracked streams in this channel
pub enum TrackStream {
    #[command(name = "add")]
    Add(TrackStreamAdd),
    #[command(name = "remove")]
    Remove(TrackStreamRemove),
    #[command(name = "list")]
    List(TrackStreamList),
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "add",
    help = "Track a twitch stream in this channel.\n\
    When the stream goes online, a notification will be send to this channel within a few minutes."
)]
/// Track a twitch stream in this channel
pub struct TrackStreamAdd {
    /// Name of the twitch channel
    name: String,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "remove")]
/// Untrack a twitch stream in this channel
pub struct TrackStreamRemove {
    /// Name of the twitch channel
    name: String,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "list")]
/// List all tracked twitch streams in this channel
pub struct TrackStreamList;

pub async fn slash_trackstream(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    match TrackStream::from_interaction(command.input_data())? {
        TrackStream::Add(add) => addstream(ctx, command.into(), add.name.as_ref()).await,
        TrackStream::Remove(remove) => removestream(ctx, command.into(), remove.name.as_ref()).await,
        TrackStream::List(_) => tracked(ctx, command.into()).await,
    }
}
