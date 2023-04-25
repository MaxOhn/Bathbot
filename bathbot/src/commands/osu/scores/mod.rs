use std::sync::Arc;

use bathbot_macros::{HasMods, HasName, SlashCommand};
use eyre::Result;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use self::{server::server_scores, user::user_scores};
use crate::{
    commands::GameModeOption,
    core::Context,
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

mod server;
mod user;

#[derive(CreateCommand, CommandModel, SlashCommand)]
#[command(
    name = "scores",
    help = "List scores that the bot has stored.\n\
    The list will only contain scores that have been cached before i.e. \
    scores of the `/top`, `/pinned`, or `/cs` commands.\n\
    Similarly beatmaps or users won't be displayed if they're not cached.\n\
    To add a missing map, you can simply `<map [map url]` \
    and for missing users it's `<profile [username]`."
)]
/// List scores that the bot has stored
pub enum Scores {
    #[command(name = "server")]
    Server(ServerScores),
    #[command(name = "user")]
    User(UserScores),
}

#[derive(CreateCommand, CommandModel, HasMods)]
#[command(name = "server", dm_permission = false)]
/// List scores of members in this server
pub struct ServerScores {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Choose how the scores should be ordered, defaults to PP
    sort: Option<ScoresOrder>,
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for
    /// excluded)
    mods: Option<String>,
    /// Specify a country (code)
    country: Option<String>,
    /// Only show scores on maps of that mapper
    mapper: Option<String>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Default)]
pub enum ScoresOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "AR", value = "ar")]
    Ar,
    #[option(name = "BPM", value = "bpm")]
    Bpm,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "CS", value = "cs")]
    Cs,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "HP", value = "hp")]
    Hp,
    #[option(name = "Length", value = "len")]
    Length,
    #[option(name = "Misses", value = "miss")]
    Misses,
    #[option(name = "OD", value = "od")]
    Od,
    #[option(name = "PP", value = "pp")]
    #[default]
    Pp,
    #[option(name = "Ranked date", value = "ranked_date")]
    RankedDate,
    #[option(name = "Score", value = "score")]
    Score,
    #[option(name = "Stars", value = "stars")]
    Stars,
}

#[derive(CreateCommand, CommandModel, HasMods, HasName)]
#[command(name = "user")]
/// List scores of a user
pub struct UserScores {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<String>,
    /// Choose how the scores should be ordered, defaults to PP
    sort: Option<ScoresOrder>,
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for
    /// excluded)
    mods: Option<String>,
    /// Only show scores on maps of that mapper
    mapper: Option<String>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

async fn slash_scores(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Scores::from_interaction(command.input_data())? {
        Scores::Server(args) => server_scores(ctx, command, args).await,
        Scores::User(args) => user_scores(ctx, command, args).await,
    }
}
