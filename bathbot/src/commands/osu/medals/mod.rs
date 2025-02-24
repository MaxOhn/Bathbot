use std::borrow::Cow;

use bathbot_macros::{HasName, SlashCommand};
use bathbot_model::MedalGroup;
use eyre::Result;
use twilight_interactions::command::{
    AutocompleteValue, CommandModel, CommandOption, CreateCommand, CreateOption,
};
use twilight_model::id::{Id, marker::UserMarker};

pub use self::{common::*, list::*, medal::*, missing::*, recent::*, stats::*};
use crate::util::{InteractionCommandExt, interaction::InteractionCommand};

mod common;
mod list;
mod medal;
mod missing;
mod recent;

pub mod stats;

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "medal",
    desc = "Info about a medal or users' medal progress",
    help = "Info about a medal or users' medal progress.\n\
    Check out [osekai](https://osekai.net/) for more info on medals."
)]
#[allow(dead_code)]
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
#[command(
    name = "common",
    desc = "Compare which of the given users achieved medals first"
)]
pub struct MedalCommon<'a> {
    #[command(desc = "Specify a username")]
    name1: Option<Cow<'a, str>>,
    #[command(desc = "Specify a username")]
    name2: Option<Cow<'a, str>>,
    #[command(desc = "Specify a medal order")]
    sort: Option<MedalCommonOrder>,
    #[command(
        desc = "Filter out some medals",
        help = "Filter out some medals.\n\
        If a medal group has been selected, only medals of that group will be shown."
    )]
    filter: Option<MedalCommonFilter>,
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
    #[option(name = "Skill & Dedication", value = "skill_dedication")]
    SkillDedication,
    #[option(name = "Hush-Hush", value = "hush_hush")]
    HushHush,
    #[option(name = "Hush-Hush (Expert)", value = "hush_hush_expert")]
    HushHushExpert,
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
    desc = "Display info about an osu! medal",
    help = "Display info about an osu! medal.\n\
    The solution, beatmaps, and comments are provided by [osekai](https://osekai.net/)."
)]
#[allow(dead_code)]
pub struct MedalInfo {
    #[command(
        autocomplete = true,
        desc = "Specify the name of a medal",
        help = "Specify the name of a medal.\n\
        Upper- and lowercase does not matter but punctuation is important."
    )]
    name: String,
}

#[derive(CommandModel)]
#[command(autocomplete = true)]
struct MedalInfo_<'a> {
    name: AutocompleteValue<Cow<'a, str>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "list", desc = "List all achieved medals of a user")]
pub struct MedalList<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = "Specify a medal order")]
    sort: Option<MedalListOrder>,
    #[command(desc = "Only show medals of this group")]
    group: Option<MedalGroup>,
    #[command(desc = "Reverse the resulting medal list")]
    reverse: Option<bool>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
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
#[command(
    name = "missing",
    desc = "Display a list of medals that a user is missing"
)]
pub struct MedalMissing<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = "Specify a medal order")]
    sort: Option<MedalMissingOrder>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
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
    desc = "Display recent medals of a user",
    help = "Display a recently acquired medal of a user.\n\
    The solution, beatmaps, and comments are provided by [osekai](https://osekai.net/)."
)]
pub struct MedalRecent<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(desc = "Specify an index e.g. 1 = most recent or `random`")]
    index: Option<Cow<'a, str>>,
    #[command(desc = "Only show medals of this group")]
    group: Option<MedalGroup>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, Default, HasName)]
#[command(name = "stats", desc = "Display medal stats for a user")]
pub struct MedalStats<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

pub async fn slash_medal(mut command: InteractionCommand) -> Result<()> {
    match Medal_::from_interaction(command.input_data())? {
        Medal_::Common(args) => common((&mut command).into(), args).await,
        Medal_::Info(args) => info((&mut command).into(), args).await,
        Medal_::List(args) => list((&mut command).into(), args).await,
        Medal_::Missing(args) => missing((&mut command).into(), args).await,
        Medal_::Recent(args) => recent((&mut command).into(), args).await,
        Medal_::Stats(args) => stats((&mut command).into(), args).await,
    }
}
