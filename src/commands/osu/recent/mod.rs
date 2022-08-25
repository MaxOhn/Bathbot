use std::{borrow::Cow, sync::Arc};

use command_macros::{HasMods, HasName, SlashCommand};
use rosu_v2::prelude::{GameMode, Grade};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::{osu::top, GameModeOption, GradeOption},
    database::ListSize,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    BotResult, Context,
};

pub use self::{leaderboard::*, list::*, score::*, simulate::*};

use self::fix::*;

use super::{FarmFilter, HasMods, ModsResult, TopArgs, TopScoreOrder};

mod fix;
mod leaderboard;
mod list;
mod score;
mod simulate;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "recent",
    help = "Retrieve a user's recent plays and display them in various forms.\n\
    The osu!api can provide the last 100 recent plays done within the last 24 hours."
)]
/// Display info about a user's recent plays
pub enum Recent<'a> {
    #[command(name = "score")]
    Score(RecentScore<'a>),
    #[command(name = "best")]
    Best(RecentBest),
    #[command(name = "leaderboard")]
    Leaderboard(RecentLeaderboard<'a>),
    #[command(name = "list")]
    List(RecentList<'a>),
    #[command(name = "fix")]
    Fix(RecentFix),
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "score",
    help = "Show a user's recent score (same as `/rs`).\n\
    To add a timestamp to a twitch VOD, be sure you linked yourself to a twitch account via `/config`."
)]
/// Show a user's recent score (same as `/rs`)
pub struct RecentScore<'a> {
    #[command(help = "Specify a gamemode.\n\
    For mania the combo will be displayed as `[ combo / ratio ]` \
    with ratio being `n320/n300`.")]
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "By default the very last play will be chosen.\n\
        However, if this index is specified, the play at that index will be displayed instead.\n\
        E.g. `index:1` is the default and `index:2` would show the second most recent play.\n\
        The given index should be between 1 and 100."
    )]
    /// Choose the recent score's index
    index: Option<usize>,
    /// Consider only scores with this grade
    grade: Option<GradeOption>,
    /// Specify whether only passes should be considered
    passes: Option<bool>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(name = "best")]
/// Display the user's current top100 sorted by date (same as `/rb`)
pub struct RecentBest {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<String>,
    #[command(help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
        or `-mods!` for excluded mods.\n\
        Examples:\n\
        - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
        - `+hdhr!`: Scores must have exactly `HDHR`\n\
        - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
        - `-nm!`: Scores can not be nomod so there must be any other mod")]
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)
    mods: Option<String>,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "By default the command will show paginated embeds containing five scores per page.\n\
        However, if this index is specified, the command will only show the score at the given index.\n\
        E.g. `index:1` will show the top score and \
        `index:3` will show the score with the third highest pp amount."
    )]
    /// Choose a specific score index
    index: Option<u32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
    /// Reverse the resulting score list
    reverse: Option<bool>,
    #[command(
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
        While ar & co will be adjusted to mods, stars will not."
    )]
    /// Specify a search query containing artist, difficulty, AR, BPM, ...
    query: Option<String>,
    /// Consider only scores with this grade
    grade: Option<GradeOption>,
    #[command(help = "Specify if you want to filter out farm maps.\n\
        A map counts as farmy if its mapset appears in the top 727 \
        sets based on how often the set is in people's top100 scores.\n\
        The list of mapsets can be checked with `/popular mapsets` or \
        on [here](https://osutracker.com/stats)")]
    /// Specify if you want to filter out farm maps
    farm: Option<FarmFilter>,
    /// Filter out all scores that don't have a perfect combo
    perfect_combo: Option<bool>,
    #[command(help = "Size of the embed.\n\
      `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1.\n\
      The default can be set with the `/config` command.")]
    /// Condense top plays
    size: Option<ListSize>,
}

impl<'a> TryFrom<RecentBest> for TopArgs<'a> {
    type Error = &'static str;

    fn try_from(args: RecentBest) -> Result<Self, Self::Error> {
        let mods = match args.mods() {
            ModsResult::Mods(mods) => Some(mods),
            ModsResult::None => None,
            ModsResult::Invalid => return Err(Self::ERR_PARSE_MODS),
        };

        Ok(Self {
            name: args.name.map(Cow::Owned),
            discord: args.discord,
            mode: args.mode.map(GameMode::from),
            mods,
            min_acc: None,
            max_acc: None,
            min_combo: None,
            max_combo: None,
            grade: args.grade.map(Grade::from),
            sort_by: TopScoreOrder::Date,
            reverse: args.reverse.unwrap_or(false),
            perfect_combo: args.perfect_combo,
            index: args.index.map(|n| n as usize),
            query: args.query,
            farm: args.farm,
            size: args.size,
            has_dash_r: false,
            has_dash_p_or_i: false,
        })
    }
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(name = "leaderboard")]
/// Show the leaderboard of a user's recently played map
pub struct RecentLeaderboard<'a> {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax, \
        e.g. `hdhr` or `+hdhr!`, and filter out all scores that don't match those mods."
    )]
    /// Specify mods e.g. hdhr or nm
    mods: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "By default the leaderboard of the very last score will be displayed.\n\
        However, if this index is specified, the leaderboard of the score at that index will be displayed instead.\n\
        E.g. `index:1` is the default and `index:2` for the second most recent play."
    )]
    /// Choose the recent score's index
    index: Option<usize>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(name = "list")]
/// Show all recent plays of a user
pub struct RecentList<'a> {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
        While ar & co will be adjusted to mods, stars will not."
    )]
    /// Specify a search query containing artist, difficulty, AR, BPM, ...
    query: Option<String>,
    /// Consider only scores with this grade
    grade: Option<GradeOption>,
    /// Specify whether only passes should be considered
    passes: Option<bool>,
    #[command(help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
    or `-mods!` for excluded mods.\n\
    Examples:\n\
    - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
    - `+hdhr!`: Scores must have exactly `HDHR`\n\
    - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
    - `-nm!`: Scores can not be nomod so there must be any other mod")]
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)
    mods: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "fix")]
/// Display a user's pp after unchoking their recent score
pub struct RecentFix {
    #[command(help = "Specify a gamemode. \
        Since combo does not matter in mania, its scores can't be fixed.")]
    /// Specify a gamemode
    mode: Option<RecentFixGameMode>,
    /// Specify a username
    name: Option<String>,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "By default the very last play will be chosen.\n\
        However, if this index is specified, the play at that index will be fixed instead.\n\
        E.g. `index:1` is the default and `index:2` would fix the second most recent play.\n\
        The given index should be between 1 and 100."
    )]
    /// Choose the recent score's index
    index: Option<usize>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum RecentFixGameMode {
    #[option(name = "osu", value = "osu")]
    Osu,
    #[option(name = "taiko", value = "taiko")]
    Taiko,
    #[option(name = "ctb", value = "ctb")]
    Catch,
}

impl From<RecentFixGameMode> for GameMode {
    #[inline]
    fn from(mode: RecentFixGameMode) -> Self {
        match mode {
            RecentFixGameMode::Osu => Self::Osu,
            RecentFixGameMode::Taiko => Self::Taiko,
            RecentFixGameMode::Catch => Self::Catch,
        }
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "simulate")]
/// Unchoke a user's recent score or simulate a perfect play on its map
pub enum RecentSimulate<'a> {
    #[command(name = "osu")]
    Osu(RecentSimulateOsu<'a>),
    #[command(name = "taiko")]
    Taiko(RecentSimulateTaiko<'a>),
    #[command(name = "ctb")]
    Catch(RecentSimulateCatch<'a>),
    #[command(name = "mania")]
    Mania(RecentSimulateMania<'a>),
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "osu",
    help = "Simulate an osu!standard score.\n\
    If no hitresults, combo, or acc are specified, it will unchoke the score."
)]
/// Simulate an osu!standard score
pub struct RecentSimulateOsu<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    pub mods: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "By default the very last play will be chosen.\n\
        However, if this index is specified, the play at that index will be chosen instead.\n\
        E.g. `index:1` is the default and `index:2` would take the second most recent play."
    )]
    /// Choose the recent score's index
    index: Option<usize>,
    #[command(min_value = 0)]
    /// Specify the amount of 300s
    pub n300: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of 100s
    pub n100: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of 50s
    pub n50: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of misses
    pub misses: Option<u32>,
    #[command(min_value = 0.0, max_value = 100.0)]
    /// Specify an accuracy
    pub acc: Option<f32>,
    #[command(min_value = 0)]
    /// Specify a combo
    pub combo: Option<u32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "taiko",
    help = "Simulate an osu!taiko score.\n\
    If no hitresults, combo, or acc are specified, it will unchoke the score."
)]
/// Simulate an osu!taiko score
pub struct RecentSimulateTaiko<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    pub mods: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "By default the very last play will be chosen.\n\
        However, if this index is specified, the play at that index will be chosen instead.\n\
        E.g. `index:1` is the default and `index:2` would take the second most recent play."
    )]
    /// Choose the recent score's index
    index: Option<usize>,
    #[command(min_value = 0)]
    /// Specify the amount of 300s
    pub n300: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of 100s
    pub n100: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of misses
    pub misses: Option<u32>,
    #[command(min_value = 0.0, max_value = 100.0)]
    /// Specify an accuracy
    pub acc: Option<f32>,
    #[command(min_value = 0)]
    /// Specify a combo
    pub combo: Option<u32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "ctb",
    help = "Simulate an osu!ctb score.\n\
    If no hitresults, combo, or acc are specified, it will unchoke the score."
)]
/// Simulate an osu!ctb score
pub struct RecentSimulateCatch<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    pub mods: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "By default the very last play will be chosen.\n\
        However, if this index is specified, the play at that index will be chosen instead.\n\
        E.g. `index:1` is the default and `index:2` would take the second most recent play."
    )]
    /// Choose the recent score's index
    index: Option<usize>,
    #[command(min_value = 0)]
    /// Specify the amount of fruit hits
    pub fruits: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of droplet hits
    pub droplets: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of tiny droplet hits
    pub tiny_droplets: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of fruit misses
    pub misses: Option<u32>,
    #[command(min_value = 0.0, max_value = 100.0)]
    /// Specify an accuracy
    pub acc: Option<f32>,
    #[command(min_value = 0)]
    /// Specify a combo
    pub combo: Option<u32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "mania",
    help = "Simulate an osu!mania score.\n\
    If `score` is not specified, a perfect play will be shown."
)]
/// Simulate an osu!mania score
pub struct RecentSimulateMania<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    pub mods: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "By default the very last play will be chosen.\n\
        However, if this index is specified, the play at that index will be chosen instead.\n\
        E.g. `index:1` is the default and `index:2` would take the second most recent play."
    )]
    /// Choose the recent score's index
    index: Option<usize>,
    #[command(
        min_value = 0,
        max_value = 1_000_000,
        help = "Mania calculations don't depend on specific hitresults, accuracy or combo.\n\
        Instead it just requires the score.\n\
        The value should be between 0 and 1,000,000 and already adjusted to mods \
        e.g. only up to 500,000 for `EZ` or up to 250,000 for `EZNF`."
    )]
    /// Specify the score
    pub score: Option<u32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[allow(unused)] // fields are used through transmute in From impl
#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(name = "rb")]
/// Display the user's current top100 sorted by date (same as `/rb`)
pub struct Rb {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<String>,
    #[command(help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
        or `-mods!` for excluded mods.\n\
        Examples:\n\
        - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
        - `+hdhr!`: Scores must have exactly `HDHR`\n\
        - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
        - `-nm!`: Scores can not be nomod so there must be any other mod")]
    /// Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)
    mods: Option<String>,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "By default the command will show paginated embeds containing five scores per page.\n\
        However, if this index is specified, the command will only show the score at the given index.\n\
        E.g. `index:1` will show the top score and \
        `index:3` will show the score with the third highest pp amount."
    )]
    /// Choose a specific score index
    index: Option<u32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
    /// Reverse the resulting score list
    reverse: Option<bool>,
    #[command(
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
        While ar & co will be adjusted to mods, stars will not."
    )]
    /// Specify a search query containing artist, difficulty, AR, BPM, ...
    query: Option<String>,
    /// Consider only scores with this grade
    grade: Option<GradeOption>,
    #[command(help = "Specify if you want to filter out farm maps.\n\
        A map counts as farmy if its mapset appears in the top 727 \
        sets based on how often the set is in people's top100 scores.\n\
        The list of mapsets can be checked with `/popular mapsets` or \
        on [here](https://osutracker.com/stats)")]
    /// Specify if you want to filter out farm maps
    farm: Option<FarmFilter>,
    /// Filter out all scores that don't have a perfect combo
    perfect_combo: Option<bool>,
    #[command(help = "Size of the embed.\n\
      `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1.\n\
      The default can be set with the `/config` command.")]
    /// Condense top plays
    size: Option<ListSize>,
}

impl From<Rb> for RecentBest {
    #[inline]
    fn from(args: Rb) -> Self {
        Self {
            mode: args.mode,
            name: args.name,
            mods: args.mods,
            index: args.index,
            discord: args.discord,
            reverse: args.reverse,
            query: args.query,
            grade: args.grade,
            farm: args.farm,
            perfect_combo: args.perfect_combo,
            size: args.size,
        }
    }
}

async fn slash_recent(ctx: Arc<Context>, mut command: InteractionCommand) -> BotResult<()> {
    match Recent::from_interaction(command.input_data())? {
        Recent::Score(args) => score(ctx, (&mut command).into(), args).await,
        Recent::Best(args) => match TopArgs::try_from(args) {
            Ok(args) => top(ctx, (&mut command).into(), args).await,
            Err(content) => {
                command.error(&ctx, content).await?;

                Ok(())
            }
        },
        Recent::Leaderboard(args) => leaderboard(ctx, (&mut command).into(), args).await,
        Recent::List(args) => list(ctx, (&mut command).into(), args).await,
        Recent::Fix(args) => fix(ctx, (&mut command).into(), args).await,
    }
}

async fn slash_rb(ctx: Arc<Context>, mut command: InteractionCommand) -> BotResult<()> {
    let args = Rb::from_interaction(command.input_data())?;

    match TopArgs::try_from(RecentBest::from(args)) {
        Ok(args) => top(ctx, (&mut command).into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}
