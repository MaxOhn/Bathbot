use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{HasName, SlashCommand};
use bathbot_model::MedalGroup;
use eyre::Result;
use twilight_interactions::command::{
    AutocompleteValue, CommandModel, CommandOption, CreateCommand, CreateOption,
};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub use self::{
    common::*, list::*, medal::handle_autocomplete as handle_medal_autocomplete, medal::*,
    missing::*, recent::*, stats::*,
};

mod common;
mod list;
mod medal;
mod missing;
mod recent;

pub mod stats;

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "medal",
    help = "Info about a medal or users' medal progress.\n\
    Check out [osekai](https://osekai.net/) for more info on medals."
)]
#[allow(dead_code)]
/// Info about a medal or users' medal progress
pub enum Medal<'a> {
    #[command(name = "common")]
    Common(MedalCommon<'a>),
    #[command(name = "info")]
    Info(MedalInfo),
    #[command(name = "list")]
    List(MedalList<'a>),
    #[command(name = "missing")]
    Missing(MedalMissing<'a>),
    #[command(name = "recent")]
    Recent(MedalRecent<'a>),
    #[command(name = "stats")]
    Stats(MedalStats<'a>),
}

#[derive(CommandModel)]
enum Medal_<'a> {
    #[command(name = "common")]
    Common(MedalCommon<'a>),
    #[command(name = "info")]
    Info(MedalInfo_<'a>),
    #[command(name = "list")]
    List(MedalList<'a>),
    #[command(name = "missing")]
    Missing(MedalMissing<'a>),
    #[command(name = "recent")]
    Recent(MedalRecent<'a>),
    #[command(name = "stats")]
    Stats(MedalStats<'a>),
}

#[derive(CommandModel, CreateCommand, Default)]
#[command(name = "common")]
/// Compare which of the given users achieved medals first
pub struct MedalCommon<'a> {
    /// Specify a username
    name1: Option<Cow<'a, str>>,
    /// Specify a username
    name2: Option<Cow<'a, str>>,
    /// Specify a medal order
    sort: Option<MedalCommonOrder>,
    #[command(help = "Filter out some medals.\n\
        If a medal group has been selected, only medals of that group will be shown.")]
    /// Filter out some medals
    filter: Option<MedalCommonFilter>,
    #[command(
        help = "Instead of specifying an osu! username with the `name1` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord1: Option<Id<UserMarker>>,
    /// Specify a linked discord user
    discord2: Option<Id<UserMarker>>,
}

#[derive(CommandOption, CreateOption)]
pub enum MedalCommonOrder {
    #[option(name = "Alphabetically", value = "alphabet")]
    Alphabet,
    #[option(name = "Date First", value = "date_first")]
    DateFirst,
    #[option(name = "Date Last", value = "date_last")]
    DateLast,
    #[option(name = "Rarity", value = "rarity")]
    Rarity,
}

#[derive(CommandOption, CreateOption)]
pub enum MedalCommonFilter {
    #[option(name = "None", value = "none")]
    None,
    #[option(name = "Unique", value = "unique")]
    Unique,
    #[option(name = "Skill", value = "skill")]
    Skill,
    #[option(name = "Dedication", value = "dedication")]
    Dedication,
    #[option(name = "Hush-Hush", value = "hush_hush")]
    HushHush,
    #[option(name = "Beatmap Packs", value = "map_packs")]
    BeatmapPacks,
    #[option(name = "Beatmap Challenge Packs", value = "map_challenge_packs")]
    BeatmapChallengePacks,
    #[option(name = "Seasonal Spotlights", value = "seasonal_spotlights")]
    SeasonalSpotlights,
    #[option(name = "Beatmap Spotlights", value = "map_spotlights")]
    BeatmapSpotlights,
    #[option(name = "Mod Introduction", value = "mod_intro")]
    ModIntroduction,
}

#[derive(CreateCommand)]
#[command(
    name = "info",
    help = "Display info about an osu! medal.\n\
        The solution, beatmaps, and comments are provided by [osekai](https://osekai.net/)."
)]
#[allow(dead_code)]
/// Display info about an osu! medal
pub struct MedalInfo {
    #[command(
        autocomplete = true,
        help = "Specify the name of a medal.\n\
        Upper- and lowercase does not matter but punctuation is important."
    )]
    /// Specify the name of a medal
    name: String,
}

#[derive(CommandModel)]
#[command(autocomplete = true)]
struct MedalInfo_<'a> {
    name: AutocompleteValue<Cow<'a, str>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "list")]
/// List all achieved medals of a user
pub struct MedalList<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    /// Specify a medal order
    sort: Option<MedalListOrder>,
    /// Only show medals of this group
    group: Option<MedalGroup>,
    /// Reverse the resulting medal list
    reverse: Option<bool>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandOption, CreateOption)]
pub enum MedalListOrder {
    #[option(name = "Alphabetically", value = "alphabet")]
    Alphabet,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Medal ID", value = "medal_id")]
    MedalId,
    #[option(name = "Rarity", value = "rarity")]
    Rarity,
}

impl Default for MedalListOrder {
    #[inline]
    fn default() -> Self {
        Self::Date
    }
}

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(name = "missing")]
/// Display a list of medals that a user is missing
pub struct MedalMissing<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    /// Specify a medal order
    sort: Option<MedalMissingOrder>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandOption, CreateOption)]
pub enum MedalMissingOrder {
    #[option(name = "Alphabetically", value = "alphabet")]
    Alphabet,
    #[option(name = "Medal ID", value = "medal_id")]
    MedalId,
    #[option(name = "Rarity", value = "rarity")]
    Rarity,
}

impl Default for MedalMissingOrder {
    #[inline]
    fn default() -> Self {
        Self::Alphabet
    }
}

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(
    name = "recent",
    help = "Display a recently acquired medal of a user.\n\
    The solution, beatmaps, and comments are provided by [osekai](https://osekai.net/)."
)]
/// Display recent medals of a user
pub struct MedalRecent<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(min_value = 1)]
    /// Specify an index e.g. 1 = most recent
    index: Option<usize>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(name = "stats")]
/// Display medal stats for a user
pub struct MedalStats<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

pub async fn slash_medal(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Medal_::from_interaction(command.input_data())? {
        Medal_::Common(args) => common(ctx, (&mut command).into(), args).await,
        Medal_::Info(args) => info(ctx, (&mut command).into(), args).await,
        Medal_::List(args) => list(ctx, (&mut command).into(), args).await,
        Medal_::Missing(args) => missing(ctx, (&mut command).into(), args).await,
        Medal_::Recent(args) => recent(ctx, (&mut command).into(), args).await,
        Medal_::Stats(args) => stats(ctx, (&mut command).into(), args).await,
    }
}
