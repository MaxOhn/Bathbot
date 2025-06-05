use std::borrow::Cow;

use bathbot_macros::{HasMods, HasName, SlashCommand};
use bathbot_model::command_fields::{GameModeOption, GradeOption};
use bathbot_psql::model::configs::ListSize;
use eyre::Result;
use rosu_v2::prelude::{GameMode, Grade};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{Id, marker::UserMarker};

use self::fix::*;
pub use self::{leaderboard::*, list::*, score::*};
use super::{HasMods, ModsResult, ScoreOrder, TopArgs, TopScoreOrder};
use crate::{
    commands::{
        DISCORD_OPTION_DESC, DISCORD_OPTION_HELP,
        osu::{LeaderboardSort, top},
    },
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

mod fix;
mod leaderboard;
mod list;
mod score;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "recent",
    desc = "Display info about a user's recent plays",
    help = "Retrieve a user's recent plays and display them in various forms.\n\
    The osu!api can provide the last 100 recent plays done within the last 24 hours."
)]
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
    desc = "Show a user's recent score (same as `/rs`)",
    help = "Show a user's recent score (same as `/rs`).\n\
    To add a timestamp to a twitch VOD, be sure you linked yourself to a twitch account via `/config`."
)]
pub struct RecentScore<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Choose the recent score's index or `random`",
        help = "By default the very last play will be chosen.\n\
        However, if this index is specified, the play at that index will be displayed instead.\n\
        E.g. `index:1` is the default and `index:2` would show the second most recent play.\n\
        The given index should be between 1 and 100 or `random`."
    )]
    index: Option<Cow<'a, str>>,
    #[command(desc = "Consider only scores with this grade")]
    grade: Option<GradeOption>,
    #[command(desc = "Specify whether only passes should be considered")]
    passes: Option<bool>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "best",
    desc = "Display the user's current top200 sorted by date (same as `/rb`)"
)]
pub struct RecentBest {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)",
        help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
        or `-mods!` for excluded mods.\n\
        Examples:\n\
        - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
        - `+hdhr!`: Scores must have exactly `HDHR`\n\
        - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
        - `-nm!`: Scores can not be nomod so there must be any other mod"
    )]
    mods: Option<String>,
    #[command(
        desc = "Choose a specific score index or `random`",
        help = "By default the command will show paginated embeds containing ten scores per page.\n\
        However, if this index is specified, the command will only show the score at the given index.\n\
        E.g. `index:1` will show the top score and \
        `index:3` will show the score with the third highest pp amount.\n\
        With `random` or `?` it'll choose a random index."
    )]
    index: Option<String>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
    #[command(desc = "Reverse the resulting score list")]
    reverse: Option<bool>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
        While ar & co will be adjusted to mods, stars will not."
    )]
    query: Option<String>,
    #[command(desc = "Consider only scores with this grade")]
    grade: Option<GradeOption>,
    #[command(desc = "Filter out all scores that don't have a perfect combo")]
    perfect_combo: Option<bool>,
    #[command(
        desc = "Condense top plays",
        help = "Size of the embed.\n\
        `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1.\n\
        The default can be set with the `/config` command."
    )]
    size: Option<ListSize>,
}

impl TryFrom<RecentBest> for TopArgs<'_> {
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
            index: args.index,
            query: args.query,
            size: args.size,
            has_dash_r: false,
            has_dash_p_or_i: false,
        })
    }
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "leaderboard",
    desc = "Show the leaderboard of a user's recently played map"
)]
pub struct RecentLeaderboard<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit `+mods!` / `+mods` syntax, \
        e.g. `hdhr` or `+hdhr!`, and filter out all scores that don't match those mods."
    )]
    mods: Option<Cow<'a, str>>,
    #[command(
        desc = "Choose how the scores should be ordered",
        help = "Choose how the scores should be ordered, defaults to `score`.\n\
        Note that the scores will still be the top pp scores, they'll just be re-ordered."
    )]
    sort: Option<LeaderboardSort>,
    #[command(
        desc = "Choose the recent score's index or `random`",
        help = "By default the leaderboard of the very last score will be displayed.\n\
        However, if this index is specified, the leaderboard of the score at that index will be displayed instead.\n\
        E.g. `index:1` is the default and `index:2` for the second most recent play.\n\
        With `random` or `?` it'll choose a random index."
    )]
    index: Option<Cow<'a, str>>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(name = "list", desc = "Show all recent plays of a user")]
pub struct RecentList<'a> {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
        While ar & co will be adjusted to mods, stars will not."
    )]
    query: Option<String>,
    #[command(desc = "Consider only scores with this grade")]
    grade: Option<GradeOption>,
    #[command(desc = "Choose how the scores should be ordered")]
    sort: Option<ScoreOrder>,
    #[command(desc = "Specify whether only passes should be considered")]
    passes: Option<bool>,
    #[command(
        desc = "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)",
        help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
        or `-mods!` for excluded mods.\n\
        Examples:\n\
        - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
        - `+hdhr!`: Scores must have exactly `HDHR`\n\
        - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
        - `-nm!`: Scores can not be nomod so there must be any other mod"
    )]
    mods: Option<Cow<'a, str>>,
    #[command(desc = "Show each map-mod pair only once")]
    unique: Option<RecentListUnique>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, CreateOption, CommandOption)]
pub enum RecentListUnique {
    #[option(name = "Highest PP", value = "pp")]
    HighestPp,
    #[option(name = "Highest Score", value = "score")]
    HighestScore,
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "fix",
    desc = "Display a user's pp after unchoking their recent score"
)]
pub struct RecentFix {
    #[command(
        desc = "Specify a gamemode",
        help = "Specify a gamemode. \
        Since combo does not matter in mania, its scores can't be fixed."
    )]
    mode: Option<RecentFixGameMode>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Choose the recent score's index or `random`",
        help = "By default the very last play will be chosen.\n\
        However, if this index is specified, the play at that index will be fixed instead.\n\
        E.g. `index:1` is the default and `index:2` would fix the second most recent play.\n\
        The given index should be between 1 and 100 or `random`."
    )]
    index: Option<String>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
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

#[allow(unused)] // fields are used through transmute in From impl
#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(name = "rb", desc = "Display the user's current top200 sorted by date")]
pub struct Rb {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)",
        help = "Filter out all scores that don't match the specified mods.\n\
        Mods must be given as `+mods` for included mods, `+mods!` for exact mods, \
        or `-mods!` for excluded mods.\n\
        Examples:\n\
        - `+hd`: Scores must have at least `HD` but can also have more other mods\n\
        - `+hdhr!`: Scores must have exactly `HDHR`\n\
        - `-ezhd!`: Scores must have neither `EZ` nor `HD` e.g. `HDDT` would get filtered out\n\
        - `-nm!`: Scores can not be nomod so there must be any other mod"
    )]
    mods: Option<String>,
    #[command(
        desc = "Choose a specific score index or `random`",
        help = "By default the command will show paginated embeds containing five scores per page.\n\
        However, if this index is specified, the command will only show the score at the given index.\n\
        E.g. `index:1` will show the top score and \
        `index:3` will show the score with the third highest pp amount.\n\
        With `random` or `?` it'll choose a random index."
    )]
    index: Option<String>,
    #[command(desc = DISCORD_OPTION_DESC, help = DISCORD_OPTION_HELP)]
    discord: Option<Id<UserMarker>>,
    #[command(desc = "Reverse the resulting score list")]
    reverse: Option<bool>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, or stars like for example `fdfd ar>10 od>=9`.\n\
        While ar & co will be adjusted to mods, stars will not."
    )]
    query: Option<String>,
    #[command(desc = "Consider only scores with this grade")]
    grade: Option<GradeOption>,
    #[command(desc = "Filter out all scores that don't have a perfect combo")]
    perfect_combo: Option<bool>,
    #[command(
        desc = "Condense top plays",
        help = "Size of the embed.\n\
        `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1.\n\
        The default can be set with the `/config` command."
    )]
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
            perfect_combo: args.perfect_combo,
            size: args.size,
        }
    }
}

async fn slash_recent(mut command: InteractionCommand) -> Result<()> {
    match Recent::from_interaction(command.input_data())? {
        Recent::Score(args) => score((&mut command).into(), args).await,
        Recent::Best(args) => match TopArgs::try_from(args) {
            Ok(args) => top((&mut command).into(), args).await,
            Err(content) => {
                command.error(content).await?;

                Ok(())
            }
        },
        Recent::Leaderboard(args) => leaderboard((&mut command).into(), args).await,
        Recent::List(args) => list((&mut command).into(), args).await,
        Recent::Fix(args) => fix((&mut command).into(), args).await,
    }
}

async fn slash_rb(mut command: InteractionCommand) -> Result<()> {
    let args = Rb::from_interaction(command.input_data())?;

    match TopArgs::try_from(RecentBest::from(args)) {
        Ok(args) => top((&mut command).into(), args).await,
        Err(content) => {
            command.error(content).await?;

            Ok(())
        }
    }
}
