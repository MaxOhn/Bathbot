use std::{borrow::Cow, cmp::Ordering};

use bathbot_macros::{command, HasMods, HasName, SlashCommand};
use bathbot_model::ScoreSlim;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher,
    numbers::round,
    osu::ModSelection,
};
use eyre::{Report, Result};
use rosu_pp_older::*;
use rosu_v2::{
    prelude::{GameMod, GameMode, OsuError, Score},
    request::UserId,
};
use time::OffsetDateTime;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use super::TopIfEntry;
use crate::{
    active::{impls::TopIfPagination, ActiveMessages},
    commands::osu::{require_link, user_not_found, HasMods, ModsResult, TopIfScoreOrder},
    core::commands::{prefix::Args, CommandOrigin},
    manager::{redis::osu::UserArgs, OsuMap},
    util::{
        interaction::InteractionCommand,
        query::{FilterCriteria, IFilterCriteria, Searchable, TopCriteria},
        ChannelExt, InteractionCommandExt,
    },
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "topold",
    desc = "How current top plays would look like in old pp systems",
    help = "Check a user's **current** top plays if their pp would be based on a previous pp system"
)]
pub enum TopOld<'a> {
    #[command(name = "osu")]
    Osu(TopOldOsu<'a>),
    #[command(name = "taiko")]
    Taiko(TopOldTaiko<'a>),
    #[command(name = "ctb")]
    Catch(TopOldCatch<'a>),
    #[command(name = "mania")]
    Mania(TopOldMania<'a>),
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "osu",
    desc = "How current osu!std top plays would look like in old pp systems",
    help = "The osu!standard pp history looks roughly like this:\n\
    - 2012: ppv1 (can't be implemented)\n\
    - 2014: [ppv2 introduction](https://osu.ppy.sh/home/news/2014-01-26-new-performance-ranking)\n\
    - 2014: 1.5x star difficulty, nerf aim, buff acc, buff length\n\
    - 2015: High CS buff, FL depends on length, \"high AR\" increased 10->10.33\n\
    - 2015: Slight high CS nerf\n\
    - 2018: [HD adjustment](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)\n\
    - 2019: [Angles, speed, spaced streams](https://osu.ppy.sh/home/news/2019-02-05-new-changes-to-star-rating-performance-points)\n\
    - 2021: [High AR nerf, NF & SO buff, speed & acc adjustment](https://osu.ppy.sh/home/news/2021-01-14-performance-points-updates)\n\
    - 2021: [Diff spike nerf, AR buff, FL-AR adjust](https://osu.ppy.sh/home/news/2021-07-27-performance-points-star-rating-updates)\n\
    - 2021: [Rhythm buff, slider buff, FL skill](https://osu.ppy.sh/home/news/2021-11-09-performance-points-star-rating-updates)\n\
    - 2022: [Aim buff, doubletap detection improvement, low AR nerf, FL adjustments](https://osu.ppy.sh/home/news/2022-09-30-changes-to-osu-sr-and-pp)"
)]
pub struct TopOldOsu<'a> {
    #[command(desc = "Choose which version should replace the current pp system")]
    version: TopOldOsuVersion,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, stars, pp, acc, score, misses, date or ranked_date \
        e.g. `ar>10 od>=9 ranked<2017-01-01 creator=monstrata acc>99 acc<=99.5`."
    )]
    query: Option<String>,
    #[command(
        desc = "Choose how the scores should be ordered",
        help = "Choose how the scores should be ordered, defaults to `pp`."
    )]
    sort: Option<TopIfScoreOrder>,
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
    #[command(desc = "Reverse the resulting score list")]
    reverse: Option<bool>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, PartialEq)]
pub enum TopOldOsuVersion {
    #[option(name = "May 2014 - July 2014", value = "may14_july14")]
    May14July14,
    #[option(name = "July 2014 - February 2015", value = "july14_february15")]
    July14February15,
    #[option(name = "February 2015 - April 2015", value = "february15_april15")]
    February15April15,
    #[option(name = "April 2015 - May 2018", value = "april15_may18")]
    April15May18,
    #[option(name = "May 2018 - February 2019", value = "may18_february19")]
    May18February19,
    #[option(name = "February 2019 - January 2021", value = "february19_january21")]
    February19January21,
    #[option(name = "January 2021 - July 2021", value = "january21_july21")]
    January21July21,
    #[option(name = "July 2021 - November 2021", value = "july21_november21")]
    July21November21,
    #[option(
        name = "November 2021 - September 2022",
        value = "november21_september22"
    )]
    November21September22,
    #[option(name = "September 2022 - Now", value = "september22_now")]
    September22Now,
}

impl TryFrom<i32> for TopOldOsuVersion {
    type Error = &'static str;

    fn try_from(year: i32) -> Result<Self, Self::Error> {
        match year {
            2007..=2011 | 7..=11 => {
                Err("Up until april 2012, ranked score was the skill metric.\n\
                The first available pp system is from 2014.")
            }
            2012..=2013 | 12..=13 => Err(
                "April 2012 till january 2014 the ppv1 system was in place, \
                which is unfortunately impossible to implement nowadays \
                because of lacking data \\:(\n\
                The first available pp system is from 2014.",
            ),
            2014 | 14 => Ok(Self::May14July14),
            2015 | 15 => Ok(Self::February15April15),
            2016..=2017 | 16..=17 => Ok(Self::April15May18),
            2018 | 18 => Ok(Self::May18February19),
            2019..=2020 | 19..=20 => Ok(Self::February19January21),
            2021 | 21 => Ok(Self::July21November21),
            2022 | 22 => Ok(Self::November21September22),
            i32::MIN..=2006 => Err("osu! was not a thing until september 2007.\n\
                The first available pp system is from 2014."),
            _ => Ok(Self::September22Now),
        }
    }
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "taiko",
    desc = "How current osu!taiko top plays would look like in old pp systems",
    help = "The osu!taiko pp history looks roughly like this:\n\
    - 2014: [ppv1](https://osu.ppy.sh/home/news/2014-03-01-performance-ranking-for-all-gamemodes)\n\
    - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-09-15-changes-to-osutaiko-star-rating)\n\
    - 2022: [Stamina, colour, & peaks rework](https://osu.ppy.sh/home/news/2022-09-28-changes-to-osu-taiko-sr-and-pp)"
)]
pub struct TopOldTaiko<'a> {
    #[command(desc = "Choose which version should replace the current pp system")]
    version: TopOldTaikoVersion,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, stars, pp, acc, score, misses, date or ranked_date \
        e.g. `ar>10 od>=9 ranked<2017-01-01 creator=monstrata acc>99 acc<=99.5`."
    )]
    query: Option<String>,
    #[command(
        desc = "Choose how the scores should be ordered",
        help = "Choose how the scores should be ordered, defaults to `pp`."
    )]
    sort: Option<TopIfScoreOrder>,
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
    #[command(desc = "Reverse the resulting score list")]
    reverse: Option<bool>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, PartialEq)]
pub enum TopOldTaikoVersion {
    #[option(name = "March 2014 - September 2020", value = "march14_september20")]
    March14September20,
    #[option(
        name = "September 2020 - September 2022",
        value = "september20_september22"
    )]
    September20September22,
    #[option(name = "September 2022 - Now", value = "september22_now")]
    September22Now,
}

impl TryFrom<i32> for TopOldTaikoVersion {
    type Error = &'static str;

    fn try_from(year: i32) -> Result<Self, Self::Error> {
        match year {
            2014..=2019 | 14..=19 => Ok(Self::March14September20),
            2020..=2022 | 20..=22 => Ok(Self::September20September22),
            i32::MIN..=2013 => Err("taiko pp were not a thing until march 2014. \
                I think? Don't quote me on that :^)"),
            _ => Ok(Self::September22Now),
        }
    }
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "ctb",
    desc = "How current osu!ctb top plays would look like in old pp systems",
    help = "The osu!ctb pp history looks roughly like this:\n\
    - 2014: [ppv1](https://osu.ppy.sh/home/news/2014-03-01-performance-ranking-for-all-gamemodes)\n\
    - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-05-14-osucatch-scoring-updates)"
)]
pub struct TopOldCatch<'a> {
    #[command(desc = "Choose which version should replace the current pp system")]
    version: TopOldCatchVersion,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, stars, pp, acc, score, misses, date or ranked_date \
        e.g. `ar>10 od>=9 ranked<2017-01-01 creator=monstrata acc>99 acc<=99.5`."
    )]
    query: Option<String>,
    #[command(
        desc = "Choose how the scores should be ordered",
        help = "Choose how the scores should be ordered, defaults to `pp`."
    )]
    sort: Option<TopIfScoreOrder>,
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
    #[command(desc = "Reverse the resulting score list")]
    reverse: Option<bool>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, PartialEq)]
pub enum TopOldCatchVersion {
    #[option(name = "March 2014 - May 2020", value = "march14_may20")]
    March14May20,
    #[option(name = "May 2020 - Now", value = "may20_now")]
    May20Now,
}

impl TryFrom<i32> for TopOldCatchVersion {
    type Error = &'static str;

    fn try_from(year: i32) -> Result<Self, Self::Error> {
        match year {
            2014..=2019 | 14..=19 => Ok(Self::March14May20),
            i32::MIN..=2013 => Err("ctb pp were not a thing until march 2014. \
                I think? Don't quote me on that :^)"),
            _ => Ok(Self::May20Now),
        }
    }
}

#[derive(CommandModel, CreateCommand, HasMods, HasName)]
#[command(
    name = "mania",
    desc = "How current osu!mania top plays would look like in old pp systems",
    help = "The osu!mania pp history looks roughly like this:\n\
    - 2014: [ppv1](https://osu.ppy.sh/home/news/2014-03-01-performance-ranking-for-all-gamemodes)\n\
    - 2018: [ppv2](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)\n\
    - 2022: [Accuracy based PP](https://osu.ppy.sh/home/news/2022-10-09-changes-to-osu-mania-sr-and-pp)"
)]
pub struct TopOldMania<'a> {
    #[command(desc = "Choose which version should replace the current pp system")]
    version: TopOldManiaVersion,
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
    #[command(
        desc = "Specify a search query containing artist, difficulty, AR, BPM, ...",
        help = "Filter out scores similarly as you filter maps in osu! itself.\n\
        You can specify the artist, creator, difficulty, title, or limit values such as \
        ar, cs, hp, od, bpm, length, stars, pp, acc, score, misses, date or ranked_date \
        e.g. `ar>10 od>=9 ranked<2017-01-01 creator=monstrata acc>99 acc<=99.5`."
    )]
    query: Option<String>,
    #[command(
        desc = "Choose how the scores should be ordered",
        help = "Choose how the scores should be ordered, defaults to `pp`."
    )]
    sort: Option<TopIfScoreOrder>,
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
    #[command(desc = "Reverse the resulting score list")]
    reverse: Option<bool>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, PartialEq)]
pub enum TopOldManiaVersion {
    #[option(name = "March 2014 - May 2018", value = "march14_may18")]
    March14May18,
    #[option(name = "May 2018 - October 2022", value = "may18_october22")]
    May18October22,
    #[option(name = "October 2022 - Now", value = "october22_now")]
    October22Now,
}

impl TryFrom<i32> for TopOldManiaVersion {
    type Error = &'static str;

    fn try_from(year: i32) -> Result<Self, Self::Error> {
        match year {
            2014..=2018 | 14..=18 => Ok(Self::March14May18),
            2019..=2022 | 19..=22 => Ok(Self::May18October22),
            i32::MIN..=2013 => Err("mania pp were not a thing until march 2014. \
                I think? Don't quote me on that :^)"),
            _ => Ok(Self::October22Now),
        }
    }
}

pub async fn slash_topold(mut command: InteractionCommand) -> Result<()> {
    let args = TopOld::from_interaction(command.input_data())?;

    topold((&mut command).into(), args).await
}

#[command]
#[desc("Display a user's top plays on different pp versions")]
#[help(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    Note that the command will **not** change scores, just recalculate their pp.\n\
    The osu!standard pp history looks roughly like this:\n\
    - 2012: ppv1 (can't be implemented)\n\
    - 2014: [ppv2 introduction](https://osu.ppy.sh/home/news/2014-01-26-new-performance-ranking)\n\
    - 2014: 1.5x star difficulty, nerf aim, buff acc, buff length\n\
    - 2015: High CS buff, FL depends on length, \"high AR\" increased 10->10.33\n\
    - 2015: Slight high CS nerf\n\
    - 2018: [HD adjustment](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)\n\
    - 2019: [Angles, speed, spaced streams](https://osu.ppy.sh/home/news/2019-02-05-new-changes-to-star-rating-performance-points)\n\
    - 2021: [High AR nerf, NF & SO buff, speed & acc adjustment](https://osu.ppy.sh/home/news/2021-01-14-performance-points-updates)\n\
    - 2021: [Diff spike nerf, AR buff, FL-AR adjust](https://osu.ppy.sh/home/news/2021-07-27-performance-points-star-rating-updates)\n\
    - 2021: [Rhythm buff, slider buff, FL skill](https://osu.ppy.sh/home/news/2021-11-09-performance-points-star-rating-updates)\n\
    - 2022: [Aim buff, doubletap detection improvement, low AR nerf, FL adjustments](https://osu.ppy.sh/home/news/2022-09-30-changes-to-osu-sr-and-pp)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2015")]
#[alias("to")]
#[group(Osu)]
async fn prefix_topold(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopOld::args(GameMode::Osu, args) {
        Ok(args) => topold(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's top mania plays on different pp versions")]
#[help(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    Note that the command will **not** change scores, just recalculate their pp.\n\
    The osu!mania pp history looks roughly like this:\n\
    - 2014: [ppv1](https://osu.ppy.sh/home/news/2014-03-01-performance-ranking-for-all-gamemodes)\n\
    - 2018: [ppv2](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)\n\
    - 2022: [Accuracy based PP](https://osu.ppy.sh/home/news/2022-10-09-changes-to-osu-mania-sr-and-pp)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2015")]
#[alias("tom")]
#[group(Mania)]
async fn prefix_topoldmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopOld::args(GameMode::Mania, args) {
        Ok(args) => topold(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's top taiko plays on different pp versions")]
#[help(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    Note that the command will **not** change scores, just recalculate their pp.\n\
    The osu!taiko pp history looks roughly like this:\n\
    - 2014: [ppv1](https://osu.ppy.sh/home/news/2014-03-01-performance-ranking-for-all-gamemodes)\n\
    - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-09-15-changes-to-osutaiko-star-rating)\n\
    - 2022: [Stamina, colour, & peaks rework](https://osu.ppy.sh/home/news/2022-09-28-changes-to-osu-taiko-sr-and-pp)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2015")]
#[alias("tot")]
#[group(Taiko)]
async fn prefix_topoldtaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopOld::args(GameMode::Taiko, args) {
        Ok(args) => topold(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's top ctb plays on different pp versions")]
#[help(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    Note that the command will **not** change scores, just recalculate their pp.\n\
    The osu!ctb pp history looks roughly like this:\n\
    - 2014: [ppv1](https://osu.ppy.sh/home/news/2014-03-01-performance-ranking-for-all-gamemodes)\n\
    - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-05-14-osucatch-scoring-updates)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2019")]
#[aliases("toc", "topoldcatch")]
#[group(Catch)]
async fn prefix_topoldctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match TopOld::args(GameMode::Catch, args) {
        Ok(args) => topold(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

impl<'m> TopOld<'m> {
    fn args(mode: GameMode, args: Args<'m>) -> Result<Self, &'static str> {
        let mut name = None;
        let mut discord = None;
        let mut year = None;

        for arg in args.take(2) {
            if let Ok(num) = arg.parse() {
                year = Some(num);
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        let year = year.unwrap_or_else(|| OffsetDateTime::now_utc().year());

        let args = match mode {
            GameMode::Osu => {
                let version = TopOldOsuVersion::try_from(year)?;

                let osu = TopOldOsu {
                    version,
                    name,
                    discord,
                    query: None,
                    sort: None,
                    mods: None,
                    reverse: None,
                };

                Self::Osu(osu)
            }
            GameMode::Taiko => {
                let version = TopOldTaikoVersion::try_from(year)?;

                let taiko = TopOldTaiko {
                    version,
                    name,
                    discord,
                    query: None,
                    sort: None,
                    mods: None,
                    reverse: None,
                };

                Self::Taiko(taiko)
            }
            GameMode::Catch => {
                let version = TopOldCatchVersion::try_from(year)?;

                let catch = TopOldCatch {
                    version,
                    name,
                    discord,
                    query: None,
                    sort: None,
                    mods: None,
                    reverse: None,
                };

                Self::Catch(catch)
            }
            GameMode::Mania => {
                let version = TopOldManiaVersion::try_from(year)?;

                let mania = TopOldMania {
                    version,
                    name,
                    discord,
                    query: None,
                    sort: None,
                    mods: None,
                    reverse: None,
                };

                Self::Mania(mania)
            }
        };

        Ok(args)
    }

    fn date_range(&self) -> &'static str {
        match self {
            TopOld::Osu(o) => match o.version {
                TopOldOsuVersion::May14July14 => "between may 2014 and july 2014",
                TopOldOsuVersion::July14February15 => "between july 2014 and february 2015",
                TopOldOsuVersion::February15April15 => "between february 2015 and april 2015",
                TopOldOsuVersion::April15May18 => "between april 2015 and may 2018",
                TopOldOsuVersion::May18February19 => "between may 2018 and february 2019",
                TopOldOsuVersion::February19January21 => "between february 2019 and january 2021",
                TopOldOsuVersion::January21July21 => "between january 2021 and july 2021",
                TopOldOsuVersion::July21November21 => "between july 2021 and november 2021",
                TopOldOsuVersion::November21September22 => {
                    "between november 2021 and september 2022"
                }
                TopOldOsuVersion::September22Now => "since september 2022",
            },
            TopOld::Taiko(t) => match t.version {
                TopOldTaikoVersion::March14September20 => "between march 2014 and september 2020",
                TopOldTaikoVersion::September20September22 => {
                    "between september 2020 and september 2022"
                }
                TopOldTaikoVersion::September22Now => "since september 2022",
            },
            TopOld::Catch(c) => match c.version {
                TopOldCatchVersion::March14May20 => "between march 2014 and may 2020",
                TopOldCatchVersion::May20Now => "since may 2020",
            },
            TopOld::Mania(m) => match m.version {
                TopOldManiaVersion::March14May18 => "between march 2014 and may 2018",
                TopOldManiaVersion::May18October22 => "between may 2018 and october 2022",
                TopOldManiaVersion::October22Now => "since october 2022",
            },
        }
    }
}

macro_rules! pp_std {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_res = $version::OsuPP::new($rosu_map).mods($mods).calculate();

        let max_pp = max_pp_res.pp as f32;
        let stars = max_pp_res.difficulty.stars as f32;
        let stats = $score.statistics.as_legacy(GameMode::Osu);

        let attrs = $version::OsuPP::new($rosu_map)
            .mods($mods)
            .attributes(max_pp_res)
            .n300(stats.count_300)
            .n100(stats.count_100)
            .n50(stats.count_50)
            .misses(stats.count_miss)
            .combo($score.max_combo)
            .calculate();

        let pp = attrs.pp as f32;
        let max_combo = attrs.max_combo() as u32;

        (pp, max_pp, stars, max_combo)
    }};
}

macro_rules! pp_ctb {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_res = $version::FruitsPP::new($rosu_map).mods($mods).calculate();

        let max_pp = max_pp_res.pp as f32;
        let stars = max_pp_res.difficulty.stars as f32;
        let stats = $score.statistics.as_legacy(GameMode::Catch);

        let attrs = $version::FruitsPP::new($rosu_map)
            .mods($mods)
            .attributes(max_pp_res)
            .fruits(stats.count_300)
            .droplets(stats.count_100)
            .tiny_droplets(stats.count_50)
            .tiny_droplet_misses(stats.count_katu)
            .misses(stats.count_miss)
            .combo($score.max_combo)
            .calculate();

        let pp = attrs.pp as f32;
        let max_combo = attrs.max_combo();

        (pp, max_pp, stars, max_combo)
    }};
}

macro_rules! pp_tko {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_res = $version::TaikoPP::new($rosu_map).mods($mods).calculate();

        let max_pp = max_pp_res.pp as f32;
        let stars = max_pp_res.difficulty.stars as f32;
        let stats = $score.statistics.as_legacy(GameMode::Taiko);

        let attrs = $version::TaikoPP::new($rosu_map)
            .mods($mods)
            .attributes(max_pp_res)
            .n300(stats.count_300)
            .n100(stats.count_100)
            .misses(stats.count_miss)
            .combo($score.max_combo)
            .calculate();

        let pp = attrs.pp as f32;
        let max_combo = attrs.max_combo();

        (pp, max_pp, stars, max_combo)
    }};
}

/// Same as `user_id!` but the args aren't passed by reference
macro_rules! user_id_ref {
    ($orig:ident, $args:ident) => {
        match crate::commands::osu::HasName::user_id($args) {
            crate::commands::osu::UserIdResult::Id(user_id) => Some(user_id),
            crate::commands::osu::UserIdResult::None => None,
            crate::commands::osu::UserIdResult::Future(fut) => match fut.await {
                crate::commands::osu::UserIdFutureResult::Id(user_id) => Some(user_id),
                crate::commands::osu::UserIdFutureResult::NotLinked(user_id) => {
                    let content = format!("<@{user_id}> is not linked to an osu!profile");

                    return $orig.error(content).await;
                }
                crate::commands::osu::UserIdFutureResult::Err(err) => {
                    let content = bathbot_util::constants::GENERAL_ISSUE;
                    let _ = $orig.error(content).await;

                    return Err(err);
                }
            },
        }
    };
}

async fn topold(orig: CommandOrigin<'_>, args: TopOld<'_>) -> Result<()> {
    let (user_id, common) = match &args {
        TopOld::Osu(args) => (user_id_ref!(orig, args), args.to_common()),
        TopOld::Taiko(args) => (user_id_ref!(orig, args), args.to_common()),
        TopOld::Catch(args) => (user_id_ref!(orig, args), args.to_common()),
        TopOld::Mania(args) => (user_id_ref!(orig, args), args.to_common()),
    };

    let Some(common) = common else {
        let content = "Failed to parse mods.\n\
            If you want included mods, specify it e.g. as `+hrdt`.\n\
            If you want exact mods, specify it e.g. as `+hdhr!`.\n\
            And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

        return orig.error(content).await;
    };

    let mode = common.mode;
    let owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(owner).await?;

    let user_id = match user_id {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };

    let legacy_scores = match config.score_data {
        Some(score_data) => score_data.is_legacy(),
        None => match orig.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| config.score_data)
                .await
                .map_or(false, ScoreData::is_legacy),
            None => false,
        },
    };

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

    // Calculate bonus pp
    let actual_pp: f32 = scores
        .iter()
        .filter_map(|score| score.weight)
        .map(|weight| weight.pp)
        .sum();

    let pre_pp = user.stats().pp();
    let bonus_pp = pre_pp - actual_pp;

    let mut entries = match process_scores(scores, &args).await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to process scores"));
        }
    };

    // Sort by adjusted pp
    entries.sort_unstable_by(|a, b| {
        b.score
            .pp
            .partial_cmp(&a.score.pp)
            .unwrap_or(Ordering::Equal)
    });

    // Calculate adjusted pp
    let adjusted_pp: f32 = entries.iter().zip(0..).fold(0.0, |sum, (entry, i)| {
        sum + entry.score.pp * 0.95_f32.powi(i)
    });

    let adjusted_pp = round(bonus_pp + adjusted_pp);
    let username = user.username();

    // Process filter/sorting afterwards
    common.process(&mut entries);

    // Accumulate all necessary data
    let mut content = format!(
        "`{username}`{plural} {mode}top100 {version}",
        plural = plural(username),
        mode = mode_str(mode),
        version = args.date_range(),
    );

    if let Some(criteria) = common.query {
        criteria.display(&mut content);
    }

    if let Some(sort) = common.sort {
        content.push_str(" (`Order: ");

        let sort_str = match sort {
            TopIfScoreOrder::Pp => "Old PP",
            TopIfScoreOrder::PpDelta => "PP delta",
            TopIfScoreOrder::Stars => "Old stars",
        };

        content.push_str(sort_str);
        content.push_str("`)");
    } else {
        content.push(':');
    }

    let pagination = TopIfPagination::builder()
        .user(user)
        .entries(entries.into_boxed_slice())
        .mode(mode)
        .pre_pp(pre_pp)
        .post_pp(adjusted_pp)
        .content(content.into_boxed_str())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

async fn process_scores(scores: Vec<Score>, args: &TopOld<'_>) -> Result<Vec<TopIfEntry>> {
    let mut entries = Vec::with_capacity(scores.len());

    let maps_id_checksum = scores
        .iter()
        .map(|score| {
            (
                score.map_id as i32,
                score.map.as_ref().and_then(|map| map.checksum.as_deref()),
            )
        })
        .collect();

    let mut maps = Context::osu_map().maps(&maps_id_checksum).await?;

    for (score, i) in scores.into_iter().zip(1..) {
        let Some(mut map) = maps.remove(&score.map_id) else {
            continue;
        };
        map = map.convert(score.mode);

        async fn use_current_system(score: &Score, map: &OsuMap) -> (f32, f32, f32, u32) {
            let attrs = Context::pp(map)
                .mode(score.mode)
                .mods(&score.mods)
                .performance()
                .await;

            let pp = score.pp.expect("missing pp");
            let max_pp = attrs.pp() as f32;
            let stars = attrs.stars() as f32;
            let max_combo = attrs.max_combo();

            (pp, max_pp, stars, max_combo)
        }

        let mods = score.mods.bits();
        let rosu_map = &map.pp_map;

        let (new_pp, max_pp, stars, max_combo) = match args {
            TopOld::Osu(o) => match o.version {
                TopOldOsuVersion::May14July14 => pp_std!(osu_2014_may, rosu_map, score, mods),
                TopOldOsuVersion::July14February15 => pp_std!(osu_2014_july, rosu_map, score, mods),
                TopOldOsuVersion::February15April15 => {
                    pp_std!(osu_2015_february, rosu_map, score, mods)
                }
                TopOldOsuVersion::April15May18 => pp_std!(osu_2015_april, rosu_map, score, mods),
                TopOldOsuVersion::May18February19 => pp_std!(osu_2018, rosu_map, score, mods),
                TopOldOsuVersion::February19January21 => pp_std!(osu_2019, rosu_map, score, mods),
                TopOldOsuVersion::January21July21 => {
                    pp_std!(osu_2021_january, rosu_map, score, mods)
                }
                TopOldOsuVersion::July21November21 => pp_std!(osu_2021_july, rosu_map, score, mods),
                TopOldOsuVersion::November21September22 => {
                    pp_std!(osu_2021_november, rosu_map, score, mods)
                }
                TopOldOsuVersion::September22Now => use_current_system(&score, &map).await,
            },
            TopOld::Taiko(t) => match t.version {
                TopOldTaikoVersion::March14September20 => {
                    pp_tko!(taiko_ppv1, rosu_map, score, mods)
                }
                TopOldTaikoVersion::September20September22 => {
                    pp_tko!(taiko_2020, rosu_map, score, mods)
                }
                TopOldTaikoVersion::September22Now => use_current_system(&score, &map).await,
            },
            TopOld::Catch(c) => match c.version {
                TopOldCatchVersion::March14May20 => pp_ctb!(fruits_ppv1, rosu_map, score, mods),
                TopOldCatchVersion::May20Now => use_current_system(&score, &map).await,
            },
            TopOld::Mania(m) => match m.version {
                TopOldManiaVersion::March14May18 => {
                    let max_pp_res = mania_ppv1::ManiaPP::new(rosu_map).mods(mods).calculate();

                    let max_pp = max_pp_res.pp as f32;
                    let stars = max_pp_res.difficulty.stars as f32;

                    let attrs = mania_ppv1::ManiaPP::new(rosu_map)
                        .mods(mods)
                        .attributes(max_pp_res)
                        .score(score.score)
                        .accuracy(score.accuracy)
                        .calculate();

                    let pp = attrs.pp as f32;
                    let max_combo = Context::pp(&map).difficulty().await.max_combo();

                    (pp, max_pp, stars, max_combo)
                }
                TopOldManiaVersion::May18October22 => {
                    let max_pp_res = mania_2018::ManiaPP::new(rosu_map).mods(mods).calculate();

                    let max_pp = max_pp_res.pp as f32;
                    let stars = max_pp_res.difficulty.stars as f32;

                    let attrs = mania_2018::ManiaPP::new(rosu_map)
                        .mods(mods)
                        .attributes(max_pp_res)
                        .score(score.score)
                        .calculate();

                    let pp = attrs.pp as f32;
                    let max_combo = Context::pp(&map).difficulty().await.max_combo();

                    (pp, max_pp, stars, max_combo)
                }
                TopOldManiaVersion::October22Now => use_current_system(&score, &map).await,
            },
        };

        let old_pp = score.pp.expect("missing pp");

        let entry = TopIfEntry {
            original_idx: i,
            score: ScoreSlim::new(score, new_pp),
            old_pp,
            map,
            stars,
            max_pp,
            max_combo,
        };

        entries.push(entry);
    }

    Ok(entries)
}

fn plural(name: &str) -> &'static str {
    match name.chars().last() {
        Some('s') => "'",
        Some(_) | None => "'s",
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Osu => "",
        GameMode::Taiko => "taiko ",
        GameMode::Catch => "ctb ",
        GameMode::Mania => "mania ",
    }
}

struct CommonArgs<'a> {
    mode: GameMode,
    sort: Option<TopIfScoreOrder>,
    mods: Option<ModSelection>,
    query: Option<FilterCriteria<TopCriteria<'a>>>,
    reverse: Option<bool>,
}

impl CommonArgs<'_> {
    fn process(&self, entries: &mut Vec<TopIfEntry>) {
        if let Some(ref criteria) = self.query {
            entries.retain(|entry| entry.matches(criteria));
        }

        if let Some(ref selection) = self.mods {
            match selection {
                ModSelection::Include(mods) | ModSelection::Exact(mods) if mods.is_empty() => {
                    entries.retain(|entry| entry.score.mods.is_empty())
                }
                ModSelection::Include(mods) => entries.retain(|entry| {
                    mods.iter()
                        .all(|gamemod| entry.score.mods.contains_intermode(gamemod))
                }),
                ModSelection::Exclude(mods) if mods.is_empty() => {
                    entries.retain(|entry| !entry.score.mods.is_empty())
                }
                ModSelection::Exclude(mods) => entries.retain(|entry| {
                    !mods
                        .iter()
                        .any(|gamemod| entry.score.mods.contains_intermode(gamemod))
                }),
                ModSelection::Exact(mods) => entries.retain(|entry| {
                    entry
                        .score
                        .mods
                        .iter()
                        .map(GameMod::intermode)
                        .eq(mods.iter())
                }),
            }
        }

        match self.sort.unwrap_or_default() {
            TopIfScoreOrder::Pp => {} // already sorted
            TopIfScoreOrder::PpDelta => {
                entries.sort_by(|a, b| b.pp_delta().total_cmp(&a.pp_delta()))
            }
            TopIfScoreOrder::Stars => entries.sort_unstable_by(|a, b| b.stars.total_cmp(&a.stars)),
        }

        if self.reverse.unwrap_or(false) {
            entries.reverse();
        }
    }
}

macro_rules! impl_from {
    ( $( $ty:ident: $mode:ident ,)* ) => {
        $(
            impl $ty<'_> {
                #[doc = "Returns `None` if the mods have an invalid format"]
                fn to_common(&self) -> Option<CommonArgs<'_>> {
                    let mods = match self.mods() {
                        ModsResult::Mods(mods) => Some(mods),
                        ModsResult::None => None,
                        ModsResult::Invalid => return None,
                    };

                    let args = CommonArgs {
                        mode: GameMode::$mode,
                        sort: self.sort,
                        mods,
                        query: self.query.as_deref().map(TopCriteria::create),
                        reverse: self.reverse,
                    };

                    Some(args)
                }
            }
        )*
    };
}

impl_from! {
    TopOldOsu: Osu,
    TopOldTaiko: Taiko,
    TopOldCatch: Catch,
    TopOldMania: Mania,
}
