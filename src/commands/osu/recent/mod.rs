use std::{borrow::Cow, sync::Arc};

use command_macros::{HasName, SlashCommand};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{util::osu::ScoreOrder, BotResult, Context};

pub use self::{leaderboard::*, list::*, score::*, simulate::*};

use self::fix::*;

use super::{prepare_score, GradeArg};

mod fix;
mod leaderboard;
mod list;
mod score;
mod simulate;

// TODO
pub fn define_rb() -> MyCommand {
    MyCommand::new("rb", "Display the user's current top100 sorted by date").options(best_options())
}

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
    Best(RecentBest<'a>),
    #[command(name = "leaderboard")]
    Leaderboard(RecentLeaderboard<'a>),
    #[command(name = "list")]
    List(RecentList<'a>),
    #[command(name = "fix")]
    Fix(RecentFix<'a>),
    #[command(name = "simulate")]
    Simulate(RecentSimulate<'a>),
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

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "best")]
/// Display the user's current top100 sorted by date (same as `/rb`)
pub struct RecentBest<'a> {
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
}

impl<'a> TryFrom<RecentBest<'a>> for TopArgs<'a> {
    type Error = &'static str;

    fn try_from(args: RecentBest<'a>) -> Result<Self, Self::Error> {
        Ok(Self {
            name: args.name,
            discord: args.discord,
            mode: (),
            mods: (),
            acc_min: None,
            acc_max: None,
            combo_min: None,
            combo_max: None,
            grade: (),
            sort_by: TopScoreOrder::Date,
            reverse: (),
            perfect_combo: args.perfect_combo,
            index: args.index,
            query: args.query,
            farm: args.farm,
            has_dash_r: false,
            has_dash_p_or_i: false,
        })
    }
}

#[derive(CommandModel, CreateCommand, HasName)]
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

#[derive(CommandModel, CreateCommand, HasName)]
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
pub struct RecentFix<'a> {
    #[command(help = "Specify a gamemode. \
        Since combo does not matter in mania, its scores can't be fixed.")]
    /// Specify a gamemode
    mode: Option<RecentFixGameMode>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
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

#[derive(CommandOption, CreateOption)]
pub enum RecentFixGameMode {
    #[option(name = "osu", value = "osu")]
    Osu,
    #[option(name = "taiko", value = "taiko")]
    Taiko,
    #[option(name = "ctb", value = "ctb")]
    Catch,
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

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "osu",
    help = "Simulate an osu!standard score.\n\
    If no hitresults, combo, or acc are specified, it will unchoke the score."
)]
/// Simulate an osu!standard score
pub struct RecentSimulateTaiko<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    mods: Option<Cow<'a, str>>,
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
    n300: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of 100s
    n100: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of 50s
    n50: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of misses
    misses: Option<u32>,
    #[command(min_value = 0.0, max_value = 100.0)]
    /// Specify an accuracy
    acc: Option<f32>,
    #[command(min_value = 0)]
    /// Specify a combo
    combo: Option<u32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
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
    mods: Option<Cow<'a, str>>,
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
    n300: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of 100s
    n100: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of misses
    misses: Option<u32>,
    #[command(min_value = 0.0, max_value = 100.0)]
    /// Specify an accuracy
    acc: Option<f32>,
    #[command(min_value = 0)]
    /// Specify a combo
    combo: Option<u32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
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
    mods: Option<Cow<'a, str>>,
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
    fruits: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of droplet hits
    droplets: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of tiny droplet hits
    tiny_droplets: Option<u32>,
    #[command(min_value = 0)]
    /// Specify the amount of fruit misses
    misses: Option<u32>,
    #[command(min_value = 0.0, max_value = 100.0)]
    /// Specify an accuracy
    acc: Option<f32>,
    #[command(min_value = 0)]
    /// Specify a combo
    combo: Option<u32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasName)]
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
    mods: Option<Cow<'a, str>>,
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
    score: Option<u32>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

async fn slash_recent(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    match Recent::from_interaction(command.input_data())? {
        Recent::Score(args) => score(ctx, command.into(), args).await,
        Recent::Best(args) => match TopArgs::try_from(args) {
            Ok(args) => top(ctx, command.into(), args).await,
            Err(content) => command.error(&ctx, content).await,
        },
        Recent::Leaderboard(args) => leaderboard(ctx, command.into(), args).await,
        Recent::List(args) => list(ctx, command.into(), args).await,
        Recent::Simulate(args) => simulate(ctx, command.into(), args).await,
        Recent::Fix(args) => fix(ctx, command.into(), args).await,
    }
}

async fn slash_rb(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = RecentBest::from_interaction(command.input_data())?;

    match TopArgs::try_from(args) {
        Ok(args) => top(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, args).await,
    }
}
