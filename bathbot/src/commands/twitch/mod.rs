use bathbot_macros::SlashCommand;
use eyre::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};

pub use self::{addstream::*, removestream::*, tracked::*};
use crate::util::{interaction::InteractionCommand, InteractionCommandExt};

pub mod addstream;
pub mod removestream;
pub mod tracked;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "trackstream",
    dm_permission = false,
    desc = "Track a twitch stream or list all tracked streams in this channel",
    help = "Track a twitch stream in this channel.\n\
    When the stream goes online, a notification will be send to this channel within a few minutes."
)]
#[flags(AUTHORITY)]
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
    desc = "Track a twitch stream in this channel",
    help = "Track a twitch stream in this channel.\n\
    When the stream goes online, a notification will be send to this channel within a few minutes."
)]
pub struct TrackStreamAdd {
    #[command(desc = "Name of the twitch channel")]
    name: String,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "remove", desc = "Untrack a twitch stream in this channel")]
pub struct TrackStreamRemove {
    #[command(desc = "Name of the twitch channel")]
    name: String,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "list",
    desc = "List all tracked twitch streams in this channel"
)]
pub struct TrackStreamList;

pub async fn slash_trackstream(mut command: InteractionCommand) -> Result<()> {
    match TrackStream::from_interaction(command.input_data())? {
        TrackStream::Add(add) => addstream((&mut command).into(), add.name.as_ref()).await,
        TrackStream::Remove(remove) => {
            removestream((&mut command).into(), remove.name.as_ref()).await
        }
        TrackStream::List(_) => tracked((&mut command).into()).await,
    }
}
