#![cfg(feature = "twitch")]

use std::sync::Arc;

use bathbot_macros::SlashCommand;
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub use self::{addstream::*, removestream::*, tracked::*};

pub mod addstream;
pub mod removestream;
pub mod tracked;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "trackstream",
    dm_permission = false,
    help = "Track a twitch stream in this channel.\n\
    When the stream goes online, a notification will be send to this channel within a few minutes."
)]
#[flags(AUTHORITY)]
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

pub async fn slash_trackstream(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match TrackStream::from_interaction(command.input_data())? {
        TrackStream::Add(add) => addstream(ctx, (&mut command).into(), add.name.as_ref()).await,
        TrackStream::Remove(remove) => {
            removestream(ctx, (&mut command).into(), remove.name.as_ref()).await
        }
        TrackStream::List(_) => tracked(ctx, (&mut command).into()).await,
    }
}
