use std::{borrow::Cow, cmp::Reverse, fmt::Write, mem};

use bathbot_macros::{command, HasMods, HasName, SlashCommand};
use bathbot_model::{
    command_fields::{GameModeOption, GradeOption},
    embed_builder::SettingsImage,
};
use bathbot_psql::model::configs::{GuildConfig, ListSize, ScoreData};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    numbers::round,
    osu::ModSelection,
    CowUtils,
};
use eyre::{Report, Result};
use rand::{thread_rng, Rng};
use rosu_v2::{
    prelude::{GameMode, Grade, OsuError, Score},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    guild::Permissions,
    id::{marker::UserMarker, Id},
};

pub use self::{if_::*, old::*};
use super::{map_strain_graph, require_link, user_not_found, HasMods, ModsResult, ScoreOrder};
use crate::{
    active::{
        impls::{SingleScoreContent, SingleScorePagination, TopPagination},
        ActiveMessages,
    },
    commands::utility::{
        MissAnalyzerCheck, ScoreEmbedDataHalf, ScoreEmbedDataPersonalBest, ScoreEmbedDataWrap,
    },
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::osu::UserArgs,
    util::{
        interaction::InteractionCommand,
        query::{IFilterCriteria, Searchable, TopCriteria},
        ChannelExt, CheckPermissions, InteractionCommandExt,
    },
    Context,
};

mod if_;
mod old;

#[derive(CommandModel, CreateCommand, HasMods, SlashCommand)]
#[command(name = "top", desc = "Display the user's current top100")]
pub struct Top {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Choose how the scores should be ordered",
        help = "Choose how the scores should be ordered, defaults to `pp`."
    )]
    sort: Option<TopScoreOrder>,
    #[command(
        desc = "Filter mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)",
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
    #[command(desc = "Choose a specific score index or `random`")]
    index: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
    #[command(desc = "Reverse the resulting score list")]
    reverse: Option<bool>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, stars, pp, acc, score, misses, date or ranked_date \
        e.g. `ar>10 od>=9 ranked<2017-01-01 creator=monstrata acc>99 acc<=99.5`."
    )]
    query: Option<String>,
    #[command(desc = "Consider only scores with this grade")]
    grade: Option<GradeOption>,
    #[command(desc = "Filter out all scores that don't have a perfect combo")]
    perfect_combo: Option<bool>,
    #[command(
        desc = "Size of the embed",
        help = "Size of the embed.\n\
        `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1.\n\
        The default can be set with the `/config` command."
    )]
    size: Option<ListSize>,
}

#[derive(Copy, Clone, Default, CommandOption, CreateOption, Eq, PartialEq)]
pub enum TopScoreOrder {
    #[option(name = "Accuracy", value = "acc")]
    Acc,
    #[option(name = "Approach Rate", value = "ar")]
    Ar,
    #[option(name = "BPM", value = "bpm")]
    Bpm,
    #[option(name = "Combo", value = "combo")]
    Combo,
    #[option(name = "Circle Size", value = "cs")]
    Cs,
    #[option(name = "Date", value = "date")]
    Date,
    #[option(name = "Drain Rate", value = "hp")]
    Hp,
    #[option(name = "Length", value = "len")]
    Length,
    #[option(name = "Map ranked date", value = "ranked_date")]
    RankedDate,
    #[option(name = "Misses", value = "miss")]
    Misses,
    #[option(name = "Overall Difficulty", value = "od")]
    Od,
    #[default]
    #[option(name = "PP", value = "pp")]
    Pp,
    #[option(name = "Score", value = "score")]
    Score,
    #[option(name = "Stars", value = "stars")]
    Stars,
}

impl From<ScoreOrder> for TopScoreOrder {
    #[inline]
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
#[aliases("topscores", "toposu", "topstd", "topstandard", "topo", "tops", "t")]
#[group(Osu)]
async fn prefix_top(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopArgs::args(None, args) {
        Ok(args) => top(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
#[alias("topm", "tm")]
#[group(Mania)]
async fn prefix_topmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopArgs::args(Some(GameMode::Mania), args) {
        Ok(args) => top(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
#[alias("topt", "tt")]
#[group(Taiko)]
async fn prefix_toptaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopArgs::args(Some(GameMode::Taiko), args) {
        Ok(args) => top(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
#[alias("topc", "topcatch", "topcatchthebeat", "tc")]
#[group(Catch)]
async fn prefix_topctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopArgs::args(Some(GameMode::Catch), args) {
        Ok(args) => top(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
#[alias(
    "rb",
    "rbo",
    "rbs",
    "recentbestosu",
    "recentbeststd",
    "recentbeststandard"
)]
#[group(Osu)]
async fn prefix_recentbest(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopArgs::args(None, args) {
        Ok(mut args) => {
            args.sort_by = TopScoreOrder::Date;

            top(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_recentbestmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopArgs::args(Some(GameMode::Mania), args) {
        Ok(mut args) => {
            args.sort_by = TopScoreOrder::Date;

            top(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_recentbesttaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopArgs::args(Some(GameMode::Taiko), args) {
        Ok(mut args) => {
            args.sort_by = TopScoreOrder::Date;

            top(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_recentbestctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopArgs::args(Some(GameMode::Catch), args) {
        Ok(mut args) => {
            args.sort_by = TopScoreOrder::Date;

            top(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

async fn slash_top(mut command: InteractionCommand) -> Result<()> {
    let args = Top::from_interaction(command.input_data())?;

    match TopArgs::try_from(args) {
        Ok(args) => top((&mut command).into(), args).await,
        Err(content) => {
            command.error(content).await?;

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
    pub index: Option<String>,
    pub query: Option<String>,
    pub size: Option<ListSize>,
    pub has_dash_r: bool,
    pub has_dash_p_or_i: bool,
}

impl<'m> TopArgs<'m> {
    const ERR_PARSE_ACC: &'static str = "Failed to parse `accuracy`.\n\
        Must be either decimal number \
        or two decimal numbers of the form `a..b` e.g. `97.5..98.5`.";
    const ERR_PARSE_COMBO: &'static str = "Failed to parse `combo`.\n\
        Must be either a positive integer \
        or two positive integers of the form `a..b` e.g. `501..1234`.";
    pub const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        If you want included mods, specify it e.g. as `+hrdt`.\n\
        If you want exact mods, specify it e.g. as `+hdhr!`.\n\
        And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

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
            index: num.to_string_opt(),
            query: None,
            size: None,
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
            index: args.index,
            query: args.query,
            size: args.size,
            has_dash_r: false,
            has_dash_p_or_i: false,
        })
    }
}

pub(super) async fn top(orig: CommandOrigin<'_>, args: TopArgs<'_>) -> Result<()> {
    let msg_owner = orig.user_id()?;

    let mut config = match Context::user_config().with_osu_id(msg_owner).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mode = args.mode.or(config.mode).unwrap_or(GameMode::Osu);

    if args.sort_by == TopScoreOrder::Pp && args.has_dash_r {
        let mode_long = mode_long(mode);
        let prefix = Context::guild_config().first_prefix(orig.guild_id()).await;

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

        return orig.error(content).await;
    } else if args.has_dash_p_or_i {
        let cmd = match args.sort_by {
            TopScoreOrder::Date => "rb",
            TopScoreOrder::Pp => "top",
            _ => unreachable!(),
        };

        let mode_long = mode_long(mode);
        let prefix = Context::guild_config().first_prefix(orig.guild_id()).await;

        let content = format!(
            "`{prefix}{cmd}{mode_long} -i / -p`? \
            Try putting the number right after the command, e.g. \
            `{prefix}{cmd}{mode_long}42`, or use the pagination buttons.",
        );

        return orig.error(content).await;
    }

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu.take() {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };

    let GuildValues {
        list_size: guild_list_size,
        render_button: guild_render_button,
        score_data: guild_score_data,
    } = match orig.guild_id() {
        Some(guild_id) => {
            Context::guild_config()
                .peek(guild_id, |config| GuildValues::from(config))
                .await
        }
        None => GuildValues::default(),
    };

    let score_data = config.score_data.or(guild_score_data).unwrap_or_default();
    let legacy_scores = score_data.is_legacy();

    // Retrieve the user and their top scores
    let user_args = UserArgs::rosu_id(&user_id, mode).await;
    let scores_fut = Context::osu_scores()
        .top(legacy_scores)
        .limit(100)
        .exec_with_user(user_args);

    let (user, scores) = match scores_fut.await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or scores");

            return Err(err);
        }
    };

    let settings = config.score_embed.unwrap_or_default();

    let mut with_render = match (guild_render_button, config.render_button) {
        (None | Some(true), None) => true,
        (None | Some(true), Some(with_render)) => with_render,
        (Some(false), _) => false,
    };

    with_render &= settings.buttons.render
        && mode == GameMode::Osu
        && orig.has_permission_to(Permissions::SEND_MESSAGES)
        && Context::ordr().is_some();

    let pre_len = scores.len();

    let entries = match process_scores(scores, &args, with_render, score_data).await {
        Ok(entries) => entries,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to process scores"));
        }
    };

    let post_len = entries.len();
    let username = user.username();

    let index = match args.index.as_deref() {
        Some("random" | "?") => (post_len > 0).then(|| thread_rng().gen_range(1..=post_len)),
        Some(n) => match n.parse::<usize>() {
            Ok(n) if n > post_len => {
                let mut content = format!("`{username}` only has {post_len} top scores");

                if pre_len > post_len {
                    let _ = write!(content, " with the specified properties");
                }

                return orig.error(content).await;
            }
            Ok(n) => Some(n),
            Err(_) => {
                let content = "Failed to parse index. \
                Must be an integer between 1 and 100 or `random` / `?`.";

                return orig.error(content).await;
            }
        },
        None => None,
    };

    let single_idx = index
        .map(|num| num.saturating_sub(1))
        .or_else(|| (post_len == 1).then_some(0));

    let entries = entries.into_boxed_slice();
    let content = write_content(username, &args, entries.len(), index);

    let list_size = args
        .size
        .or(config.list_size)
        .or(guild_list_size)
        .unwrap_or_default();

    let condensed_list = match (single_idx, list_size) {
        (Some(_), _) | (None, ListSize::Single) => {
            let content = match (single_idx, content) {
                (Some(idx), Some(content)) => SingleScoreContent::OnlyForIndex { idx, content },
                (None, Some(content)) => SingleScoreContent::SameForAll(content),
                (_, None) => SingleScoreContent::None,
            };

            let graph = match single_idx.map_or_else(|| entries.first(), |idx| entries.get(idx)) {
                Some(entry) if matches!(settings.image, SettingsImage::ImageWithStrains) => {
                    let entry = entry.get_half();

                    let fut = map_strain_graph(
                        &entry.map.pp_map,
                        entry.score.mods.clone(),
                        entry.map.cover(),
                    );

                    match fut.await {
                        Ok(graph) => Some((SingleScorePagination::IMAGE_NAME.to_owned(), graph)),
                        Err(err) => {
                            warn!(?err, "Failed to create strain graph");

                            None
                        }
                    }
                }
                Some(_) | None => None,
            };

            let mut pagination = SingleScorePagination::new(
                &user, entries, settings, score_data, msg_owner, content,
            );

            if let Some(idx) = single_idx {
                pagination.set_index(idx);
            }

            return ActiveMessages::builder(pagination)
                .start_by_update(true)
                .attachment(graph)
                .begin(orig)
                .await;
        }
        (None, ListSize::Condensed) => true,
        (None, ListSize::Detailed) => false,
    };

    let pagination = TopPagination::builder()
        .user(user)
        .mode(mode)
        .entries(entries)
        .sort_by(args.sort_by)
        .condensed_list(condensed_list)
        .score_data(score_data)
        .content(content.unwrap_or_default().into_boxed_str())
        .msg_owner(msg_owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

async fn process_scores(
    scores: Vec<Score>,
    args: &TopArgs<'_>,
    with_render: bool,
    score_data: ScoreData,
) -> Result<Vec<ScoreEmbedDataWrap>> {
    let legacy_scores = score_data.is_legacy();
    let mut entries = Vec::<ScoreEmbedDataWrap>::with_capacity(scores.len());

    let acc_range = match (args.min_acc, args.max_acc) {
        (None, None) => None,
        (None, Some(max)) => Some(0.0..=max),
        (Some(min), None) => Some(min..=100.0),
        (Some(min), Some(max)) => Some(min..=max),
    };

    let combo_range = match (args.min_combo, args.max_combo) {
        (None, None) => None,
        (None, Some(max)) => Some(0..=max),
        (Some(min), None) => Some(min..=u32::MAX),
        (Some(min), Some(max)) => Some(min..=max),
    };

    let filter_criteria = args.query.as_deref().map(TopCriteria::create);

    let maps_id_checksum = scores
        .iter()
        .filter(|score| match acc_range {
            Some(ref range) => range.contains(&score.accuracy),
            None => true,
        })
        .filter(|score| match combo_range {
            Some(ref range) => range.contains(&score.max_combo),
            None => true,
        })
        .filter(|score| match args.grade {
            Some(grade) => score.grade.eq_letter(grade),
            None => true,
        })
        .filter(|score| match args.mods {
            None => true,
            Some(ref selection) => selection.filter_score(score),
        })
        .map(|score| {
            (
                score.map_id as i32,
                score.map.as_ref().and_then(|map| map.checksum.as_deref()),
            )
        })
        .collect();

    let mut maps = Context::osu_map().maps(&maps_id_checksum).await?;

    for (i, score) in scores.into_iter().enumerate() {
        let Some(mut map) = maps.remove(&score.map_id) else {
            continue;
        };

        map = map.convert(score.mode);

        let map_checksum = score
            .map
            .as_ref()
            .filter(|_| score.replay)
            .and_then(|map| map.checksum.clone());

        let pb_idx = Some(ScoreEmbedDataPersonalBest::from_index(i));

        let half = ScoreEmbedDataHalf::new(
            score,
            map,
            map_checksum,
            pb_idx,
            legacy_scores,
            with_render,
            MissAnalyzerCheck::without(),
        )
        .await;

        if let Some(ref criteria) = filter_criteria {
            if half.matches(criteria) {
                entries.push(half.into());
            }
        } else {
            entries.push(half.into());
        }
    }

    if let Some(perfect_combo) = args.perfect_combo {
        entries.retain(|entry| {
            perfect_combo == (entry.get_half().max_combo == entry.get_half().score.max_combo)
        });
    }

    match args.sort_by {
        TopScoreOrder::Acc => entries.sort_by(|a, b| {
            b.get_half()
                .score
                .accuracy
                .total_cmp(&a.get_half().score.accuracy)
        }),
        TopScoreOrder::Ar => {
            entries.sort_by(|a, b| b.get_half().ar().total_cmp(&a.get_half().ar()))
        }
        TopScoreOrder::Bpm => entries.sort_by(|a, b| {
            let a_bpm =
                a.get_half().map.bpm() as f64 * a.get_half().score.mods.clock_rate().unwrap_or(1.0);
            let b_bpm =
                b.get_half().map.bpm() as f64 * b.get_half().score.mods.clock_rate().unwrap_or(1.0);

            b_bpm.total_cmp(&a_bpm)
        }),
        TopScoreOrder::Combo => {
            entries.sort_by_key(|entry| Reverse(entry.get_half().score.max_combo))
        }
        TopScoreOrder::Cs => {
            entries.sort_by(|a, b| b.get_half().cs().total_cmp(&a.get_half().cs()))
        }
        TopScoreOrder::Date => {
            entries.sort_by_key(|entry| Reverse(entry.get_half().score.ended_at))
        }
        TopScoreOrder::Hp => {
            entries.sort_by(|a, b| b.get_half().hp().total_cmp(&a.get_half().hp()))
        }
        TopScoreOrder::Length => {
            entries.sort_by(|a, b| {
                let a_len = a.get_half().map.seconds_drain() as f64
                    / a.get_half().score.mods.clock_rate().unwrap_or(1.0);
                let b_len = b.get_half().map.seconds_drain() as f64
                    / b.get_half().score.mods.clock_rate().unwrap_or(1.0);

                b_len.total_cmp(&a_len)
            });
        }
        TopScoreOrder::Misses => entries.sort_by(|a, b| {
            let a = a.get_half();
            let b = b.get_half();

            b.score
                .statistics
                .miss
                .cmp(&a.score.statistics.miss)
                .then_with(|| {
                    let hits_a = a.score.total_hits();
                    let hits_b = b.score.total_hits();

                    let ratio_a = a.score.statistics.miss as f32 / hits_a as f32;
                    let ratio_b = b.score.statistics.miss as f32 / hits_b as f32;

                    ratio_b
                        .total_cmp(&ratio_a)
                        .then_with(|| hits_b.cmp(&hits_a))
                })
        }),
        TopScoreOrder::Od => {
            entries.sort_by(|a, b| b.get_half().od().total_cmp(&a.get_half().od()))
        }
        TopScoreOrder::Pp => {
            entries.sort_by(|a, b| b.get_half().score.pp.total_cmp(&a.get_half().score.pp))
        }
        TopScoreOrder::RankedDate => {
            entries.sort_by_key(|entry| Reverse(entry.get_half().map.ranked_date()))
        }
        TopScoreOrder::Score if score_data == ScoreData::LazerWithClassicScoring => {
            entries.sort_by_key(|entry| Reverse(entry.get_half().score.classic_score))
        }
        TopScoreOrder::Score => entries.sort_by_key(|entry| Reverse(entry.get_half().score.score)),
        TopScoreOrder::Stars => {
            entries.sort_by(|a, b| b.get_half().stars.total_cmp(&a.get_half().stars))
        }
    }

    if args.reverse {
        entries.reverse();
    }

    Ok(entries)
}

fn mode_long(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Osu => "",
        GameMode::Mania => "mania",
        GameMode::Taiko => "taiko",
        GameMode::Catch => "ctb",
    }
}

fn write_content(
    name: &str,
    args: &TopArgs<'_>,
    amount: usize,
    index: Option<usize>,
) -> Option<String> {
    let condition = args.min_acc.is_some()
        || args.max_acc.is_some()
        || args.min_combo.is_some()
        || args.max_combo.is_some()
        || args.grade.is_some()
        || args.mods.is_some()
        || args.perfect_combo.is_some()
        || args.query.is_some();

    if condition {
        Some(content_with_condition(args, amount))
    } else {
        let genitive = if name.ends_with('s') { "" } else { "s" };
        let reverse = if args.reverse { "reversed " } else { "" };

        let ordinal_suffix = match index {
            Some(n) if n % 10 == 1 && n != 11 => "st",
            Some(n) if n % 10 == 2 && n != 12 => "nd",
            Some(n) if n % 10 == 3 && n != 13 => "rd",
            _ => "th",
        };

        let content = match args.sort_by {
            TopScoreOrder::Acc => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}accuracy:")
            }
            TopScoreOrder::Ar => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}AR:")
            }
            TopScoreOrder::Bpm => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}BPM:")
            }
            TopScoreOrder::Combo => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}combo:")
            }
            TopScoreOrder::Cs => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}CS:")
            }
            TopScoreOrder::Date if (args.reverse && index.is_some_and(|n| n <= 1)) => {
                format!("Oldest score in `{name}`'{genitive} top100:")
            }
            TopScoreOrder::Date if (args.reverse && index.is_some_and(|n| n > 1)) => {
                format!(
                    "{index_string}{ordinal_suffix} oldest score in `{name}`'{genitive} top100:",
                    index_string = index.unwrap()
                )
            }
            TopScoreOrder::Date if args.reverse => {
                format!("Oldest scores in `{name}`'{genitive} top100:")
            }
            TopScoreOrder::Date if index.is_some_and(|n| n <= 1) => {
                format!("Most recent score in `{name}`'{genitive} top100:")
            }
            TopScoreOrder::Date if index.is_some_and(|n| n > 1) => {
                format!("{index_string}{ordinal_suffix} most recent score in `{name}`'{genitive} top100:", index_string = index.unwrap())
            }
            TopScoreOrder::Date => {
                format!("Most recent scores in `{name}`'{genitive} top100:")
            }
            TopScoreOrder::Hp => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}HP:")
            }
            TopScoreOrder::Length => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}length:")
            }
            TopScoreOrder::Misses => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}miss count:")
            }
            TopScoreOrder::Od => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}OD:")
            }
            TopScoreOrder::Pp if !args.reverse => return None,
            TopScoreOrder::Pp => {
                format!("`{name}`'{genitive} top100 sorted by reversed pp:")
            }
            TopScoreOrder::RankedDate => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}ranked date:")
            }
            TopScoreOrder::Score => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}score:")
            }
            TopScoreOrder::Stars => {
                format!("`{name}`'{genitive} top100 sorted by {reverse}stars:")
            }
        };

        Some(content)
    }
}

fn content_with_condition(args: &TopArgs<'_>, amount: usize) -> String {
    let mut content = String::with_capacity(64);

    match args.sort_by {
        TopScoreOrder::Acc => content.push_str("`Order: Accuracy"),
        TopScoreOrder::Ar => content.push_str("`Order: AR"),
        TopScoreOrder::Bpm => content.push_str("`Order: BPM"),
        TopScoreOrder::Combo => content.push_str("`Order: Combo"),
        TopScoreOrder::Cs => content.push_str("`Order: CS"),
        TopScoreOrder::Date => content.push_str("`Order: Date"),
        TopScoreOrder::Hp => content.push_str("`Order: HP"),
        TopScoreOrder::Length => content.push_str("`Order: Length"),
        TopScoreOrder::Misses => content.push_str("`Order: Miss count"),
        TopScoreOrder::Od => content.push_str("`Order: OD"),
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
            let _ = write!(content, " • `Acc: 0% - {}%`", round(max));
        }
        (Some(min), None) => {
            let _ = write!(content, " • `Acc: {}% - 100%`", round(min));
        }
        (Some(min), Some(max)) => {
            let _ = write!(content, " • `Acc: {}% - {}%`", round(min), round(max));
        }
    }

    match (args.min_combo, args.max_combo) {
        (None, None) => {}
        (None, Some(max)) => {
            let _ = write!(content, " • `Combo: 0 - {max}`");
        }
        (Some(min), None) => {
            let _ = write!(content, " • `Combo: {min} - ∞`");
        }
        (Some(min), Some(max)) => {
            let _ = write!(content, " • `Combo: {min} - {max}`");
        }
    }

    if let Some(grade) = args.grade {
        let _ = write!(content, " • `Grade: {grade}`");
    }

    if let Some(ref selection) = args.mods {
        let (pre, mods) = match selection {
            ModSelection::Include(mods) => ("Include ", mods),
            ModSelection::Exclude(mods) => ("Exclude ", mods),
            ModSelection::Exact(mods) => ("", mods),
        };

        let _ = write!(content, " • `Mods: {pre}{mods}`");
    }

    if let Some(perfect_combo) = args.perfect_combo {
        let _ = write!(content, " • `Perfect combo: {perfect_combo}`");
    }

    if let Some(query) = args.query.as_deref() {
        TopCriteria::create(query).display(&mut content);
    }

    let plural = if amount == 1 { "" } else { "s" };
    let _ = write!(content, "\nFound {amount} matching top score{plural}:");

    content
}

#[derive(Default)]
struct GuildValues {
    list_size: Option<ListSize>,
    render_button: Option<bool>,
    score_data: Option<ScoreData>,
}

impl From<&GuildConfig> for GuildValues {
    fn from(config: &GuildConfig) -> Self {
        Self {
            list_size: config.list_size,
            render_button: config.render_button,
            score_data: config.score_data,
        }
    }
}
