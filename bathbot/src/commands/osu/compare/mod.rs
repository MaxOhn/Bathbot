use std::borrow::Cow;

use bathbot_macros::SlashCommand;
use bathbot_model::command_fields::{GameModeOption, GradeOption};
use eyre::Result;
use twilight_interactions::command::{
    AutocompleteValue, CommandModel, CommandOption, CreateCommand, CreateOption,
};
use twilight_model::id::{Id, marker::UserMarker};

pub use self::{
    common::*,
    most_played::*,
    profile::*,
    score::{slash_compare as slash_compare_score, *},
};
use crate::{commands::{DISCORD_OPTION_DESC, DISCORD_OPTION_HELP}, util::{interaction::InteractionCommand, InteractionCommandExt}};

mod common;
mod most_played;
mod profile;
mod score;

const AT_LEAST_ONE: &str = "You need to specify at least one osu username. \
    If you're not linked, you must specify two names.";

#[derive(CreateCommand, SlashCommand)]
#[command(name = "compare", desc = "Compare scores or profiles")]
#[allow(dead_code)]
pub enum Compare<'a> {
    #[command(name = "score")]
    Score(CompareScore<'a>),
    #[command(name = "profile")]
    Profile(CompareProfile<'a>),
    #[command(name = "top")]
    Top(CompareTop<'a>),
    #[command(name = "mostplayed")]
    MostPlayed(CompareMostPlayed<'a>),
}

#[derive(CommandModel)]
pub enum CompareAutocomplete<'a> {
    #[command(name = "score")]
    Score(CompareScoreAutocomplete<'a>),
    #[command(name = "profile")]
    Profile(CompareProfile<'a>),
    #[command(name = "top")]
    Top(CompareTop<'a>),
    #[command(name = "mostplayed")]
    MostPlayed(CompareMostPlayed<'a>),
}

#[derive(CreateCommand)]
#[command(
    name = "score",
    desc = "Compare a score (same as `/cs`)",
    help = "Given a user and a map, display the user's scores on the map.\n\
    Its shorter alias is the `/cs` command."
)]
#[allow(dead_code)]
pub struct CompareScore<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a map url or map id",
        help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find."
    )]
    map: Option<Cow<'a, str>>,
    #[command(
        autocomplete = true,
        desc = "Specify a difficulty name of the map's mapset"
    )]
    difficulty: Option<String>,
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Choose how the scores should be ordered")]
    sort: Option<ScoreOrder>,
    #[command(
        desc = "Filter out scores based on mods \
        (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)",
        help = "Filter out scores based on mods.\n\
        Mods must be given as `+mods` to require these mods to be included, \
        `+mods!` to require exactly these mods, \
        or `-mods!` to ignore scores containing any of these mods.\n\
        Examples:\n\
        - `+hd`: Remove scores that don't include `HD`\n\
        - `+hdhr!`: Only keep the `HDHR` score\n\
        - `+nm!`: Only keep the nomod score\n\
        - `-ezhd!`: Remove all scores that have either `EZ` or `HD`"
    )]
    mods: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 50,
        desc = "While checking the channel history, I will choose the index-th map I can find"
    )]
    index: Option<u32>,
    #[command(desc = "Consider only scores with this grade")]
    grade: Option<GradeOption>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel)]
#[command(autocomplete = true)]
pub struct CompareScoreAutocomplete<'a> {
    pub name: Option<Cow<'a, str>>,
    pub map: Option<Cow<'a, str>>,
    pub difficulty: AutocompleteValue<String>,
    pub mode: Option<GameModeOption>,
    pub sort: Option<ScoreOrder>,
    pub mods: Option<Cow<'a, str>>,
    pub index: Option<u32>,
    pub grade: Option<GradeOption>,
    pub discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum ScoreOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Misses", value = "miss")]
    Misses,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Score", value = "score")]
    Score,
    #[option(name = "Stars", value = "stars")]
    Stars,
}

impl Default for ScoreOrder {
    #[inline]
    fn default() -> Self {
        Self::Pp
    }
}

#[derive(CommandModel, CreateCommand, Default)]
#[command(
    name = "profile",
    desc = "Compare two profiles (same as `/pc`)",
    help = "Compare profile stats between two players.\n\
    Its shorter alias is the `/pc` command.
    Note:\n\
    - PC peak = Monthly playcount peak\n\
    - PP spread = PP difference between the top score and the 100th score"
)]
pub struct CompareProfile<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name1: Option<Cow<'a, str>>,
    #[command(desc = "Specify a username")]
    name2: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name1` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord1: Option<Id<UserMarker>>,
    #[command(desc = "Specify a linked discord user")]
    discord2: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, Default)]
#[command(
    name = "top",
    desc = "Compare common top scores (same as `/ct`)",
    help = "Compare common top scores between players and see who did better on them\n\
    Its shorter alias is the `/ct` command."
)]
pub struct CompareTop<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name1: Option<Cow<'a, str>>,
    #[command(desc = "Specify a username")]
    name2: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name1` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord1: Option<Id<UserMarker>>,
    #[command(desc = "Specify a linked discord user")]
    discord2: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, Default)]
#[command(
    name = "mostplayed",
    desc = "Compare most played maps",
    help = "Compare most played maps between players and see who played them more"
)]
pub struct CompareMostPlayed<'a> {
    #[command(desc = "Specify a username")]
    name1: Option<Cow<'a, str>>,
    #[command(desc = "Specify a username")]
    name2: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name1` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord1: Option<Id<UserMarker>>,
    #[command(desc = "Specify a linked discord user")]
    discord2: Option<Id<UserMarker>>,
}

async fn slash_compare(mut command: InteractionCommand) -> Result<()> {
    match CompareAutocomplete::from_interaction(command.input_data())? {
        CompareAutocomplete::Score(args) => slash_compare_score(&mut command, args).await,
        CompareAutocomplete::Profile(args) => profile((&mut command).into(), args).await,
        CompareAutocomplete::Top(args) => top((&mut command).into(), args).await,
        CompareAutocomplete::MostPlayed(args) => mostplayed((&mut command).into(), args).await,
    }
}
