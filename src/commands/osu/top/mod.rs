use std::{borrow::Cow, fmt::Write, mem, sync::Arc};

use command_macros::{command, HasMods, HasName, SlashCommand};
use eyre::Report;
use hashbrown::HashMap;
use rkyv::{Deserialize, Infallible};
use rosu_v2::prelude::{
    GameMode, GameMods, Grade, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
    Score, User,
};
use tokio::time::{sleep, Duration};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        GameModeOption, GradeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    custom_client::OsuTrackerMapsetEntry,
    database::{EmbedsSize, MinimizedPp},
    embeds::TopSingleEmbed,
    pagination::{TopCondensedPagination, TopPagination},
    tracking::process_osu_tracking,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSUTRACKER_ISSUE, OSU_API_ISSUE},
        matcher, numbers,
        osu::{ModSelection, SortableScore},
        query::{FilterCriteria, Searchable},
        ApplicationCommandExt, ChannelExt, CowUtils, MessageExt,
    },
    BotResult, Context,
};

pub use self::{if_::*, old::*};

use super::{require_link, HasMods, ModsResult, ScoreOrder};

mod if_;
mod old;

#[derive(CommandModel, CreateCommand, HasMods, SlashCommand)]
#[command(name = "top")]
/// Display the user's current top100
pub struct Top {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<String>,
    #[command(help = "Choose how the scores should be ordered, defaults to `pp`.")]
    /// Choose how the scores should be ordered
    sort: Option<TopScoreOrder>,
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
    #[command(min_value = 1, max_value = 100)]
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
    /// Condenses the embed
    condensed: Option<bool>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Eq, PartialEq)]
pub enum TopScoreOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "BPM", value = "bpm")]
    Bpm,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Common farm", value = "farm")]
    Farm,
    #[option(name = "Length", value = "len")]
    Length,
    #[option(name = "Map ranked date", value = "ranked_date")]
    RankedDate,
    #[option(name = "Misses", value = "miss")]
    Misses,
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Score", value = "score")]
    Score,
    #[option(name = "Stars", value = "stars")]
    Stars,
}

impl Default for TopScoreOrder {
    fn default() -> Self {
        Self::Pp
    }
}

impl From<ScoreOrder> for TopScoreOrder {
    fn from(sort_by: ScoreOrder) -> Self {
        match sort_by {
            ScoreOrder::Acc => Self::Acc,
            ScoreOrder::Bpm => Self::Bpm,
            ScoreOrder::Combo => Self::Combo,
            ScoreOrder::Date => Self::Date,
            ScoreOrder::Length => Self::Length,
            ScoreOrder::Misses => Self::Misses,
            ScoreOrder::Pp => Self::Pp,
            ScoreOrder::RankedDate => Self::RankedDate,
            ScoreOrder::Score => Self::Score,
            ScoreOrder::Stars => Self::Stars,
        }
    }
}

#[derive(CommandOption, CreateOption)]
pub enum FarmFilter {
    #[option(name = "No farm", value = "no_farm")]
    NoFarm,
    #[option(name = "Only farm", value = "only_farm")]
    OnlyFarm,
}

#[command]
#[desc("Display a user's top plays")]
#[help(
    "Display a user's top plays.\n\
     Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
     There are also multiple options you can set by specifying `key=value`.\n\
     These are the keys with their values:\n\
     - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
     - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
     - `grade`: `SS`, `S`, `A`, `B`, `C`, or `D`\n\
     - `sort`: `acc`, `combo`, `date` (= `rb` command), `length`, or `position` (default)\n\
     - `reverse`: `true` or `false` (default)\n\
     \n\
     Instead of showing the scores in a list, you can also __show a single score__ by \
     specifying a number right after the command, e.g. `<top2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] \
    [grade=SS/S/A/B/C/D] [sort=acc/combo/date/length/position] [reverse=true/false]"
)]
#[examples(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr sort=combo",
    "vaxei -dt! combo=1234 sort=length",
    "peppy combo=200..500 grade=B reverse=true"
)]
#[aliases("topscores", "osutop")]
#[group(Osu)]
async fn prefix_top(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopArgs::args(None, args) {
        Ok(args) => top(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's top mania plays")]
#[help(
    "Display a user's top mania plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: `SS`, `S`, `A`, `B`, `C`, or `D`\n\
    - `sort`: `acc`, `combo`, `date` (= `rbm` command), `length`, or `position` (default)\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<topm2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] \
    [grade=SS/S/A/B/C/D] [sort=acc/combo/date/length/position] [reverse=true/false]"
)]
#[examples(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr sort=combo",
    "vaxei -dt! combo=1234 sort=length",
    "peppy combo=200..500 grade=B reverse=true"
)]
#[alias("topm")]
#[group(Mania)]
async fn prefix_topmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopArgs::args(Some(GameMode::Mania), args) {
        Ok(args) => top(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's top taiko plays")]
#[help(
    "Display a user's top taiko plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: `SS`, `S`, `A`, `B`, `C`, or `D`\n\
    - `sort`: `acc`, `combo`, `date` (= `rbt` command), `length`, or `position` (default)\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<topt2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] \
    [grade=SS/S/A/B/C/D] [sort=acc/combo/date/length/position] [reverse=true/false]"
)]
#[examples(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr sort=combo",
    "vaxei -dt! combo=1234 sort=length",
    "peppy combo=200..500 grade=B reverse=true"
)]
#[alias("topt")]
#[group(Taiko)]
async fn prefix_toptaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopArgs::args(Some(GameMode::Taiko), args) {
        Ok(args) => top(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's top ctb plays")]
#[help(
    "Display a user's top ctb plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: `SS`, `S`, `A`, `B`, `C`, or `D`\n\
    - `sort`: `acc`, `combo`, `date` (= `rbc` command), `length`, or `position` (default)\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<topc2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] \
   [grade=SS/S/A/B/C/D] [sort=acc/combo/date/length/position] [reverse=true/false]"
)]
#[examples(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr sort=combo",
    "vaxei -dt! combo=1234 sort=length",
    "peppy combo=200..500 grade=B reverse=true"
)]
#[alias("topc")]
#[group(Catch)]
async fn prefix_topctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopArgs::args(Some(GameMode::Catch), args) {
        Ok(args) => top(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Sort a user's top plays by date")]
#[help(
    "Display a user's most recent top plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: `SS`, `S`, `A`, `B`, `C`, or `D`\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<rb2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] [grade=SS/S/A/B/C/D] [reverse=true/false]"
)]
#[examples(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr",
    "vaxei -dt! combo=1234",
    "peppy combo=200..500 grade=B reverse=true"
)]
#[alias("rb")]
#[group(Osu)]
async fn prefix_recentbest(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopArgs::args(None, args) {
        Ok(mut args) => {
            args.sort_by = TopScoreOrder::Date;

            top(ctx, msg.into(), args).await
        }
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Sort a user's top mania plays by date")]
#[help(
    "Display a user's most recent top mania plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: `SS`, `S`, `A`, `B`, `C`, or `D`\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<rbm2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] [grade=SS/S/A/B/C/D] [reverse=true/false]"
)]
#[examples(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr",
    "vaxei -dt! combo=1234",
    "peppy combo=200..500 grade=B reverse=true"
)]
#[alias("rbm")]
#[group(Mania)]
async fn prefix_recentbestmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopArgs::args(Some(GameMode::Mania), args) {
        Ok(mut args) => {
            args.sort_by = TopScoreOrder::Date;

            top(ctx, msg.into(), args).await
        }
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Sort a user's top taiko plays by date")]
#[help(
    "Display a user's most recent top taiko plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: `SS`, `S`, `A`, `B`, `C`, or `D`\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<rbt2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] [grade=SS/S/A/B/C/D] [reverse=true/false]"
)]
#[examples(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr",
    "vaxei -dt! combo=1234",
    "peppy combo=200..500 grade=B reverse=true"
)]
#[alias("rbt")]
#[group(Taiko)]
async fn prefix_recentbesttaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopArgs::args(Some(GameMode::Taiko), args) {
        Ok(mut args) => {
            args.sort_by = TopScoreOrder::Date;

            top(ctx, msg.into(), args).await
        }
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Sort a user's top ctb plays by date")]
#[help(
    "Display a user's most recent top ctb plays.\n\
    Mods can be specified through the usual `+_`, `+_!`, `-_!` syntax.\n\
    There are also multiple options you can set by specifying `key=value`.\n\
    These are the keys with their values:\n\
    - `acc`: single number or two numbers of the form `a..b` e.g. `acc=97.5..98`\n\
    - `combo`: single integer or two integers of the form `a..b` e.g. `combo=500..1234`\n\
    - `grade`: `SS`, `S`, `A`, `B`, `C`, or `D`\n\
    - `reverse`: `true` or `false` (default)\n\
    \n\
    Instead of showing the scores in a list, you can also __show a single score__ by \
    specifying a number right after the command, e.g. `<rbc2 badewanne3`."
)]
#[usage(
    "[username] [mods] [acc=number[..number]] [combo=integer[..integer]] [grade=SS/S/A/B/C/D] [reverse=true/false]"
)]
#[examples(
    "badewanne3 acc=97.34..99.5 grade=A +hdhr",
    "vaxei -dt! combo=1234",
    "peppy combo=200..500 grade=B reverse=true"
)]
#[alias("rbc")]
#[group(Catch)]
async fn prefix_recentbestctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopArgs::args(Some(GameMode::Catch), args) {
        Ok(mut args) => {
            args.sort_by = TopScoreOrder::Date;

            top(ctx, msg.into(), args).await
        }
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_top(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = Top::from_interaction(command.input_data())?;

    match TopArgs::try_from(args) {
        Ok(args) => top(ctx, command.into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[derive(HasName)]
pub struct TopArgs<'a> {
    pub name: Option<Cow<'a, str>>,
    pub discord: Option<Id<UserMarker>>,
    pub mode: Option<GameMode>,
    pub mods: Option<ModSelection>,
    pub min_acc: Option<f32>,
    pub max_acc: Option<f32>,
    pub min_combo: Option<u32>,
    pub max_combo: Option<u32>,
    pub grade: Option<Grade>,
    pub sort_by: TopScoreOrder,
    pub reverse: bool,
    pub perfect_combo: Option<bool>,
    pub index: Option<usize>,
    pub query: Option<String>,
    pub farm: Option<FarmFilter>,
    pub condensed: Option<bool>,
    pub has_dash_r: bool,
    pub has_dash_p_or_i: bool,
}

impl<'m> TopArgs<'m> {
    pub const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        If you want included mods, specify it e.g. as `+hrdt`.\n\
        If you want exact mods, specify it e.g. as `+hdhr!`.\n\
        And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

    const ERR_PARSE_ACC: &'static str = "Failed to parse `accuracy`.\n\
        Must be either decimal number \
        or two decimal numbers of the form `a..b` e.g. `97.5..98.5`.";

    const ERR_PARSE_COMBO: &'static str = "Failed to parse `combo`.\n\
        Must be either a positive integer \
        or two positive integers of the form `a..b` e.g. `501..1234`.";

    fn args(mode: Option<GameMode>, args: Args<'m>) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut discord = None;
        let mut mods = None;
        let mut acc_min = None;
        let mut acc_max = None;
        let mut combo_min = None;
        let mut combo_max = None;
        let mut grade = None;
        let mut sort_by = None;
        let mut reverse = None;
        let mut has_dash_r = None;
        let mut has_dash_p_or_i = None;
        let num = args.num;

        for arg in args.map(|arg| arg.cow_to_ascii_lowercase()) {
            if arg.as_ref() == "-r" {
                has_dash_r = Some(true);
            } else if matches!(arg.as_ref(), "-p" | "-i") {
                has_dash_p_or_i = Some(true);
            } else if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "acc" | "accuracy" | "a" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let mut min = if bot.is_empty() {
                                0.0
                            } else if let Ok(num) = bot.parse::<f32>() {
                                num.clamp(0.0, 100.0)
                            } else {
                                return Err(Self::ERR_PARSE_ACC.into());
                            };

                            let mut max = if top.is_empty() {
                                100.0
                            } else if let Ok(num) = top.parse::<f32>() {
                                num.clamp(0.0, 100.0)
                            } else {
                                return Err(Self::ERR_PARSE_ACC.into());
                            };

                            if min > max {
                                mem::swap(&mut min, &mut max);
                            }

                            acc_min = Some(min);
                            acc_max = Some(max);
                        }
                        None => match value.parse() {
                            Ok(num) => acc_min = Some(num),
                            Err(_) => return Err(Self::ERR_PARSE_ACC.into()),
                        },
                    },
                    "combo" | "c" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let mut min = if bot.is_empty() {
                                0
                            } else if let Ok(num) = bot.parse() {
                                num
                            } else {
                                return Err(Self::ERR_PARSE_COMBO.into());
                            };

                            let mut max = top.parse().ok();

                            if let Some(ref mut max) = max {
                                if min > *max {
                                    mem::swap(&mut min, max);
                                }
                            }

                            combo_min = Some(min);
                            combo_max = max;
                        }
                        None => match value.parse() {
                            Ok(num) => combo_min = Some(num),
                            Err(_) => return Err(Self::ERR_PARSE_COMBO.into()),
                        },
                    },
                    "grade" | "g" => match value.parse::<GradeOption>() {
                        Ok(grade_) => grade = Some(grade_.into()),
                        Err(content) => return Err(content.into()),
                    },
                    "sort" | "s" | "order" | "ordering" => match value {
                        "acc" | "a" | "accuracy" => sort_by = Some(ScoreOrder::Acc),
                        "combo" | "c" => sort_by = Some(ScoreOrder::Combo),
                        "date" | "d" | "recent" | "r" => sort_by = Some(ScoreOrder::Date),
                        "length" | "len" | "l" => sort_by = Some(ScoreOrder::Length),
                        "pp" | "p" => sort_by = Some(ScoreOrder::Pp),
                        _ => {
                            let content = "Failed to parse `sort`.\n\
                            Must be either `acc`, `combo`, `date`, `length`, or `pp`";

                            return Err(content.into());
                        }
                    },
                    "mods" => match matcher::get_mods(value) {
                        Some(mods_) => mods = Some(mods_),
                        None => return Err(Self::ERR_PARSE_MODS.into()),
                    },
                    "reverse" | "r" => match value {
                        "true" | "t" | "1" => reverse = Some(true),
                        "false" | "f" | "0" => reverse = Some(false),
                        _ => {
                            let content =
                                "Failed to parse `reverse`. Must be either `true` or `false`.";

                            return Err(content.into());
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\n\
                            Available options are: `acc`, `combo`, `sort`, `grade`, or `reverse`."
                        );

                        return Err(content.into());
                    }
                }
            } else if let Some(mods_) = matcher::get_mods(arg.as_ref()) {
                mods = Some(mods_);
            } else {
                match matcher::get_mention_user(arg.as_ref()) {
                    Some(id) => discord = Some(id),
                    None => name = Some(arg),
                }
            }
        }

        let args = Self {
            name,
            discord,
            mode,
            mods,
            min_acc: acc_min,
            max_acc: acc_max,
            min_combo: combo_min,
            max_combo: combo_max,
            grade,
            sort_by: sort_by.unwrap_or_default().into(),
            reverse: reverse.unwrap_or(false),
            perfect_combo: None,
            index: num.map(|n| n as usize),
            query: None,
            farm: None,
            condensed: None,
            has_dash_r: has_dash_r.unwrap_or(false),
            has_dash_p_or_i: has_dash_p_or_i.unwrap_or(false),
        };

        Ok(args)
    }
}

impl TryFrom<Top> for TopArgs<'static> {
    type Error = &'static str;

    fn try_from(args: Top) -> Result<Self, Self::Error> {
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
            sort_by: args.sort.unwrap_or_default(),
            reverse: args.reverse.unwrap_or(false),
            perfect_combo: args.perfect_combo,
            index: args.index.map(|n| n as usize),
            query: args.query,
            farm: args.farm,
            condensed: args.condensed,
            has_dash_r: false,
            has_dash_p_or_i: false,
        })
    }
}

const FARM_CUTOFF: usize = 727;

pub(super) async fn top(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: TopArgs<'_>,
) -> BotResult<()> {
    if args.index.filter(|n| *n > 100).is_some() {
        let content = "Can't have more than 100 top scores.";

        return orig.error(&ctx, content).await;
    }

    let mut config = match ctx.user_config(orig.user_id()?).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mode = args.mode.or(config.mode).unwrap_or(GameMode::Osu);

    if args.sort_by == TopScoreOrder::Pp && args.has_dash_r {
        let mode_long = mode_long(mode);
        let prefix = ctx.guild_first_prefix(orig.guild_id()).await;

        let mode_short = match mode {
            GameMode::Osu => "",
            GameMode::Mania => "m",
            GameMode::Taiko => "t",
            GameMode::Catch => "c",
        };

        let content = format!(
            "`{prefix}top{mode_long} -r`? I think you meant `{prefix}recentbest{mode_long}` \
            or `{prefix}rb{mode_short}` for short ;)",
        );

        return orig.error(&ctx, content).await;
    } else if args.has_dash_p_or_i {
        let cmd = match args.sort_by {
            TopScoreOrder::Date => "rb",
            TopScoreOrder::Pp => "top",
            _ => unreachable!(),
        };

        let mode_long = mode_long(mode);
        let prefix = ctx.guild_first_prefix(orig.guild_id()).await;

        let content = format!(
            "`{prefix}{cmd}{mode_long} -i / -p`? \
            Try putting the number right after the command, e.g. \
            `{prefix}{cmd}{mode_long}42`, or use the pagination buttons.",
        );

        return orig.error(&ctx, content).await;
    }

    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match config.osu.take() {
            Some(osu) => osu.into_username(),
            None => return require_link(&ctx, &orig).await,
        },
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(&name, mode);
    let score_args = ScoreArgs::top(100).with_combo();

    let farm_fut = async {
        if args.farm.is_some() || matches!(args.sort_by, TopScoreOrder::Farm) {
            ctx.redis()
                .osutracker_stats()
                .await
                .map(|stats| {
                    stats
                        .get()
                        .mapset_count
                        .iter()
                        .map(|entry| entry.deserialize(&mut Infallible).unwrap())
                        .enumerate()
                        .map(|(i, entry): (_, OsuTrackerMapsetEntry)| {
                            (entry.mapset_id, (entry, i < FARM_CUTOFF))
                        })
                        .collect::<Farm>()
                })
                .map(Some)
                .transpose()
        } else {
            None
        }
    };

    let (user_score_result, farm_result) =
        tokio::join!(get_user_and_scores(&ctx, user_args, &score_args), farm_fut);

    let (mut user, mut scores) = match user_score_result {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let farm = match farm_result {
        Some(Ok(mapsets)) => mapsets,
        Some(Err(err)) => {
            let _ = orig.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.into());
        }
        None => HashMap::new(),
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    // Filter scores according to mods, combo, acc, and grade
    let scores = filter_scores(&ctx, scores, &args, &farm).await;

    if args.index.filter(|n| *n > scores.len()).is_some() {
        let content = format!(
            "`{name}` only has {} top scores with the specified properties",
            scores.len()
        );

        return orig.error(&ctx, content).await;
    }

    match (args.index, scores.len()) {
        (Some(num), _) => {
            let embeds_size = match (config.embeds_size, orig.guild_id()) {
                (Some(size), _) => size,
                (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
                (None, None) => EmbedsSize::default(),
            };

            let minimized_pp = match (config.minimized_pp, orig.guild_id()) {
                (Some(pp), _) => pp,
                (None, Some(guild)) => ctx.guild_minimized_pp(guild).await,
                (None, None) => MinimizedPp::default(),
            };

            let num = num.saturating_sub(1);
            single_embed(
                ctx,
                orig,
                user,
                scores,
                num,
                embeds_size,
                minimized_pp,
                None,
            )
            .await?;
        }
        (_, 1) => {
            let embeds_size = match (config.embeds_size, orig.guild_id()) {
                (Some(size), _) => size,
                (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
                (None, None) => EmbedsSize::default(),
            };

            let minimized_pp = match (config.minimized_pp, orig.guild_id()) {
                (Some(pp), _) => pp,
                (None, Some(guild)) => ctx.guild_minimized_pp(guild).await,
                (None, None) => MinimizedPp::default(),
            };

            let content = write_content(&name, &args, 1);
            single_embed(
                ctx,
                orig,
                user,
                scores,
                0,
                embeds_size,
                minimized_pp,
                content,
            )
            .await?;
        }
        (None, _) => {
            let content = write_content(&name, &args, scores.len());
            if args.condensed.unwrap_or(false) {
                condensed_paginated_embed(ctx, orig, user, scores, args.sort_by, content, farm)
                    .await?;
            } else {
                paginated_embed(ctx, orig, user, scores, args.sort_by, content, farm).await?;
            }
        }
    }

    Ok(())
}

async fn filter_scores(
    ctx: &Context,
    scores: Vec<Score>,
    args: &TopArgs<'_>,
    farm: &Farm,
) -> Vec<(usize, Score)> {
    let mut scores_indices: Vec<_> = scores.into_iter().enumerate().collect();

    if let Some(perfect_combo) = args.perfect_combo {
        scores_indices.retain(
            |(_, score)| match score.map.as_ref().and_then(|m| m.max_combo) {
                Some(combo) => perfect_combo == (combo == score.max_combo),
                None => false,
            },
        );
    }

    if let Some(grade) = args.grade {
        scores_indices.retain(|(_, score)| score.grade.eq_letter(grade));
    }

    match args.mods {
        None => {}
        Some(ModSelection::Include(mods @ GameMods::NoMod) | ModSelection::Exact(mods)) => {
            scores_indices.retain(|(_, score)| score.mods == mods)
        }
        Some(ModSelection::Include(mods)) => {
            scores_indices.retain(|(_, score)| score.mods.contains(mods))
        }
        Some(ModSelection::Exclude(GameMods::NoMod)) => {
            scores_indices.retain(|(_, score)| !score.mods.is_empty())
        }
        Some(ModSelection::Exclude(mods)) => {
            scores_indices.retain(|(_, score)| score.mods.intersection(mods).is_empty())
        }
    }

    if args.min_acc.or(args.max_acc).is_some() {
        let min = args.min_acc.unwrap_or(0.0);
        let max = args.max_acc.unwrap_or(100.0);
        scores_indices.retain(|(_, score)| (min..=max).contains(&score.accuracy));
    }

    if args.min_combo.or(args.max_combo).is_some() {
        let min = args.min_combo.unwrap_or(0);
        let max = args.max_combo.unwrap_or(u32::MAX);
        scores_indices.retain(|(_, score)| (min..=max).contains(&score.max_combo));
    }

    if let Some(query) = args.query.as_deref() {
        let criteria = FilterCriteria::new(query);
        scores_indices.retain(|(_, score)| score.matches(&criteria));
    }

    match args.farm {
        Some(FarmFilter::OnlyFarm) => scores_indices.retain(|(_, score)| {
            farm.get(&score.mapset.as_ref().unwrap().mapset_id)
                .map_or(false, |(_, farm)| *farm)
        }),
        Some(FarmFilter::NoFarm) => scores_indices.retain(|(_, score)| {
            farm.get(&score.mapset.as_ref().unwrap().mapset_id)
                .map_or(true, |(_, farm)| !*farm)
        }),
        None => {}
    }

    match args.sort_by {
        TopScoreOrder::Farm => scores_indices.sort_unstable_by(|(_, a), (_, b)| {
            let mapset_a = a.mapset_id();
            let mapset_b = b.mapset_id();

            let count_a = farm.get(&mapset_a).map_or(0, |(entry, _)| entry.count);
            let count_b = farm.get(&mapset_b).map_or(0, |(entry, _)| entry.count);

            count_b.cmp(&count_a)
        }),
        other => {
            let sort = match other {
                TopScoreOrder::Acc => ScoreOrder::Acc,
                TopScoreOrder::Bpm => ScoreOrder::Bpm,
                TopScoreOrder::Combo => ScoreOrder::Combo,
                TopScoreOrder::Date => ScoreOrder::Date,
                TopScoreOrder::Length => ScoreOrder::Length,
                TopScoreOrder::RankedDate => ScoreOrder::RankedDate,
                TopScoreOrder::Misses => ScoreOrder::Misses,
                TopScoreOrder::Pp => ScoreOrder::Pp,
                TopScoreOrder::Score => ScoreOrder::Score,
                TopScoreOrder::Stars => ScoreOrder::Stars,
                _ => unreachable!(),
            };

            sort.apply(ctx, &mut scores_indices).await
        }
    }

    if args.reverse {
        scores_indices.reverse();
    }

    scores_indices
}

fn mode_long(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Osu => "",
        GameMode::Mania => "mania",
        GameMode::Taiko => "taiko",
        GameMode::Catch => "ctb",
    }
}

#[allow(clippy::too_many_arguments)]
async fn single_embed(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    user: User,
    scores: Vec<(usize, Score)>,
    idx: usize,
    embeds_size: EmbedsSize,
    minimized_pp: MinimizedPp,
    content: Option<String>,
) -> BotResult<()> {
    let (idx, score) = scores.get(idx).unwrap();
    let map = score.map.as_ref().unwrap();

    // Prepare retrieval of the map's global top 50 and the user's top 100
    let global_idx = match map.status {
        Ranked | Loved | Qualified | Approved => {
            // TODO: Add .limit(50) when supported by osu!api
            match ctx.osu().beatmap_scores(map.map_id).await {
                Ok(scores) => scores.iter().position(|s| s == score),
                Err(err) => {
                    let report = Report::new(err).wrap_err("failed to get global scores");
                    warn!("{report:?}");

                    None
                }
            }
        }
        _ => None,
    };

    let embed_data =
        TopSingleEmbed::new(&user, score, Some(*idx), global_idx, minimized_pp, &ctx).await?;

    // Only maximize if config allows it
    match embeds_size {
        EmbedsSize::AlwaysMinimized => {
            let mut builder = MessageBuilder::new().embed(embed_data.into_minimized());

            if let Some(content) = content {
                builder = builder.content(content);
            }

            orig.create_message(&ctx, &builder).await?;
        }
        EmbedsSize::InitialMaximized => {
            let mut builder = MessageBuilder::new().embed(embed_data.as_maximized());

            if let Some(ref content) = content {
                builder = builder.content(content);
            }

            let response = orig.create_message(&ctx, &builder).await?.model().await?;

            ctx.store_msg(response.id);

            // Minimize embed after delay
            tokio::spawn(async move {
                sleep(Duration::from_secs(45)).await;

                if !ctx.remove_msg(response.id) {
                    return;
                }

                let mut builder = MessageBuilder::new().embed(embed_data.into_minimized());

                if let Some(content) = content {
                    builder = builder.content(content);
                }

                if let Err(err) = response.update(&ctx, &builder).await {
                    let report = Report::new(err).wrap_err("failed to minimize top message");
                    warn!("{report:?}");
                }
            });
        }
        EmbedsSize::AlwaysMaximized => {
            let mut builder = MessageBuilder::new().embed(embed_data.as_maximized());

            if let Some(content) = content {
                builder = builder.content(content);
            }

            orig.create_message(&ctx, &builder).await?;
        }
    }

    Ok(())
}

type Farm = HashMap<u32, (OsuTrackerMapsetEntry, bool)>;

async fn paginated_embed(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    user: User,
    scores: Vec<(usize, Score)>,
    sort_by: TopScoreOrder,
    content: Option<String>,
    farm: Farm,
) -> BotResult<()> {
    TopPagination::builder(user, scores, sort_by, farm)
        .content(content.unwrap_or_default())
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}

async fn condensed_paginated_embed(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    user: User,
    scores: Vec<(usize, Score)>,
    sort_by: TopScoreOrder,
    content: Option<String>,
    farm: Farm,
) -> BotResult<()> {
    TopCondensedPagination::builder(user, scores, sort_by, farm)
        .content(content.unwrap_or_default())
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}

fn write_content(name: &str, args: &TopArgs<'_>, amount: usize) -> Option<String> {
    let condition = args.min_acc.is_some()
        || args.max_acc.is_some()
        || args.min_combo.is_some()
        || args.max_combo.is_some()
        || args.grade.is_some()
        || args.mods.is_some()
        || args.perfect_combo.is_some()
        || args.query.is_some()
        || args.farm.is_some();

    if condition {
        Some(content_with_condition(args, amount))
    } else {
        let genitive = if name.ends_with('s') { "" } else { "s" };
        let reverse = if args.reverse { "reversed " } else { "" };
        let condensed = if args.condensed.unwrap_or(false) {
            "condensed "
        } else {
            ""
        };

        let content = match args.sort_by {
            TopScoreOrder::Farm if args.reverse => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by least popular farm:")
            }
            TopScoreOrder::Farm => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by most popular farm:")
            }
            TopScoreOrder::Acc => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by {reverse}accuracy:")
            }
            TopScoreOrder::Bpm => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by {reverse}BPM:")
            }
            TopScoreOrder::Combo => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by {reverse}combo:")
            }
            TopScoreOrder::Date if args.reverse => {
                format!("Oldest {condensed}scores in `{name}`'{genitive} top100:")
            }
            TopScoreOrder::Date => {
                format!("Most recent {condensed}scores in `{name}`'{genitive} top100:")
            }
            TopScoreOrder::Length => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by {reverse}length:")
            }
            TopScoreOrder::Misses => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by {reverse}miss count:")
            }
            TopScoreOrder::Pp if !args.reverse => return None,
            TopScoreOrder::Pp => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by reversed pp:")
            }
            TopScoreOrder::RankedDate => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by {reverse}ranked date:")
            }
            TopScoreOrder::Score => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by {reverse}score:")
            }
            TopScoreOrder::Stars => {
                format!("`{name}`'{genitive} {condensed}top100 sorted by {reverse}stars:")
            }
        };

        Some(content)
    }
}

fn content_with_condition(args: &TopArgs<'_>, amount: usize) -> String {
    let mut content = String::with_capacity(64);

    match args.sort_by {
        TopScoreOrder::Farm => content.push_str("`Order: Farm`"),
        TopScoreOrder::Acc => content.push_str("`Order: Accuracy"),
        TopScoreOrder::Bpm => content.push_str("`Order: BPM"),
        TopScoreOrder::Combo => content.push_str("`Order: Combo"),
        TopScoreOrder::Date => content.push_str("`Order: Date"),
        TopScoreOrder::Length => content.push_str("`Order: Length"),
        TopScoreOrder::Misses => content.push_str("`Order: Miss count"),
        TopScoreOrder::Pp => content.push_str("`Order: Pp"),
        TopScoreOrder::RankedDate => content.push_str("`Order: Ranked date"),
        TopScoreOrder::Score => content.push_str("`Order: Score"),
        TopScoreOrder::Stars => content.push_str("`Order: Stars"),
    }

    if args.reverse {
        content.push_str(" (reverse)`");
    } else {
        content.push('`');
    }

    match (args.min_acc, args.max_acc) {
        (None, None) => {}
        (None, Some(max)) => {
            let _ = write!(content, " ~ `Acc: 0% - {}%`", numbers::round(max));
        }
        (Some(min), None) => {
            let _ = write!(content, " ~ `Acc: {}% - 100%`", numbers::round(min));
        }
        (Some(min), Some(max)) => {
            let _ = write!(
                content,
                " ~ `Acc: {}% - {}%`",
                numbers::round(min),
                numbers::round(max)
            );
        }
    }

    match (args.min_combo, args.max_combo) {
        (None, None) => {}
        (None, Some(max)) => {
            let _ = write!(content, " ~ `Combo: 0 - {max}`");
        }
        (Some(min), None) => {
            let _ = write!(content, " ~ `Combo: {min} - ∞`");
        }
        (Some(min), Some(max)) => {
            let _ = write!(content, " ~ `Combo: {min} - {max}`");
        }
    }

    if let Some(grade) = args.grade {
        let _ = write!(content, " ~ `Grade: {grade}`");
    }

    if let Some(selection) = args.mods {
        let (pre, mods) = match selection {
            ModSelection::Include(mods) => ("Include ", mods),
            ModSelection::Exclude(mods) => ("Exclude ", mods),
            ModSelection::Exact(mods) => ("", mods),
        };

        let _ = write!(content, " ~ `Mods: {pre}{mods}`");
    }

    if let Some(perfect_combo) = args.perfect_combo {
        let _ = write!(content, " ~ `Perfect combo: {perfect_combo}`");
    }

    if let Some(query) = args.query.as_deref() {
        let _ = write!(content, " ~ `Query: {query}`");
    }

    match args.farm {
        Some(FarmFilter::OnlyFarm) => content.push_str(" ~ `Only farm`"),
        Some(FarmFilter::NoFarm) => content.push_str(" ~ `Without farm`"),
        None => {}
    }

    let plural = if amount == 1 { "" } else { "s" };
    let _ = write!(content, "\nFound {amount} matching top score{plural}:");

    content
}
