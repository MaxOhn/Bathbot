use std::{borrow::Cow, sync::Arc};

use chrono::{Datelike, Utc};
use command_macros::{command, HasName, SlashCommand};
use eyre::Report;
use rosu_pp::{Beatmap, BeatmapExt, PerformanceAttributes};
use rosu_pp_older::*;
use rosu_v2::prelude::{GameMode, OsuError, Score};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::osu::{get_user_and_scores, require_link, ScoreArgs, ScoreOrder, UserArgs},
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, TopIfEmbed},
    error::PpError,
    pagination::{Pagination, TopIfPagination},
    tracking::process_osu_tracking,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, numbers,
        osu::prepare_beatmap_file,
        ApplicationCommandExt, ChannelExt,
    },
    BotResult, Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "topold",
    help = "Check a user's **current** top plays if their pp would be based on a previous pp system"
)]
/// How the current top plays would look like on a previous pp system
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

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "osu",
    help = "The osu!standard pp history looks roughly like this:\n\
    - 2012: ppv1 (can't be implemented)\n\
    - 2014: ppv2 (unavailable)\n\
    - 2015: High CS nerf(?)\n\
    - 2018: [HD adjustment](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)\n\
    - 2019: [Angles, speed, spaced streams](https://osu.ppy.sh/home/news/2019-02-05-new-changes-to-star-rating-performance-points)\n\
    - 2021: [High AR nerf, NF & SO buff, speed & acc adjustment](https://osu.ppy.sh/home/news/2021-01-14-performance-points-updates)\n\
    - 2021: [Diff spike nerf, AR buff, FL-AR adjust](https://osu.ppy.sh/home/news/2021-07-27-performance-points-star-rating-updates)\n\
    - 2021: [Rhythm buff, slider buff, FL skill](https://osu.ppy.sh/home/news/2021-11-09-performance-points-star-rating-updates)"
)]
/// How the current osu!standard top plays would look like on a previous pp system
pub struct TopOldOsu<'a> {
    /// Choose which version should replace the current pp system
    version: TopOldOsuVersion,
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

#[derive(CommandOption, CreateOption)]
pub enum TopOldOsuVersion {
    #[option(name = "April 2015 - May 2018", value = "april15_may18")]
    April15May18,
    #[option(name = "May 2018 - February 2019", value = "may18_february19")]
    May18February19,
    #[option(name = "Feburary 2019 - January 2021", value = "feburary19_january21")]
    February19January21,
    #[option(name = "January 2021 - July 2021", value = "january21_july21")]
    January21July21,
    #[option(name = "July 2021 - November 2021", value = "july21_november21")]
    July21November21,
    #[option(name = "November 2021 - Now", value = "november21_now")]
    November21Now,
}

impl TryFrom<i32> for TopOldOsuVersion {
    type Error = &'static str;

    fn try_from(year: i32) -> Result<Self, Self::Error> {
        match year {
            i32::MIN..=2006 => Err("osu! was not a thing until september 2007.\n\
                The first available pp system is from 2015."),
            2007..=2011 => Err("Up until april 2012, ranked score was the skill metric.\n\
                The first available pp system is from 2015."),
            2012..=2013 => Err(
                "April 2012 till january 2014 the ppv1 system was in place.\n\
                The source code is not available though \\:(\n\
                The first available pp system is from 2015.",
            ),
            2014 => Err(
                "ppv2 replaced ppv1 in january 2014 and lasted until april 2015.\n\
                The source code is not available though \\:(\n\
                The first available pp system is from 2015.",
            ),
            2015..=2017 => Ok(Self::April15May18),
            2018 => Ok(Self::May18February19),
            2019..=2020 => Ok(Self::February19January21),
            2021 => Ok(Self::July21November21),
            _ => Ok(Self::November21Now),
        }
    }
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "taiko",
    help = "The osu!taiko pp history looks roughly like this:\n\
    - 2014: ppv1\n\
    - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-09-15-changes-to-osutaiko-star-rating)"
)]
/// How the current osu!taiko top plays would look like on a previous pp system
pub struct TopOldTaiko<'a> {
    /// Choose which version should replace the current pp system
    version: TopOldTaikoVersion,
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

#[derive(CommandOption, CreateOption)]
pub enum TopOldTaikoVersion {
    #[option(name = "March 2014 - September 2020", value = "march14_september20")]
    March14September20,
    #[option(name = "September 2020 - Now", value = "september20_now")]
    September20Now,
}

impl TryFrom<i32> for TopOldTaikoVersion {
    type Error = &'static str;

    fn try_from(year: i32) -> Result<Self, Self::Error> {
        match year {
            i32::MIN..=2013 => Err("taiko pp were not a thing until march 2014. \
                I think? Don't quote me on that :^)"),
            2014..=2019 => Ok(Self::March14September20),
            _ => Ok(Self::September20Now),
        }
    }
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "ctb",
    help = "The osu!ctb pp history looks roughly like this:\n\
    - 2014: ppv1\n\
    - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-05-14-osucatch-scoring-updates)"
)]
/// How the current osu!ctb top plays would look like on a previous pp system
pub struct TopOldCatch<'a> {
    /// Choose which version should replace the current pp system
    version: TopOldCatchVersion,
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

#[derive(CommandOption, CreateOption)]
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
            i32::MIN..=2013 => Err("ctb pp were not a thing until march 2014. \
                I think? Don't quote me on that :^)"),
            2014..=2019 => Ok(Self::March14May20),
            _ => Ok(Self::May20Now),
        }
    }
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(
    name = "mania",
    help = "The osu!mania pp history looks roughly like this:\n\
    - 2014: ppv1\n\
    - 2018: [ppv2](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)"
)]
/// How the current osu!mania top plays would look like on a previous pp system
pub struct TopOldMania<'a> {
    /// Choose which version should replace the current pp system
    version: TopOldManiaVersion,
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

#[derive(CommandOption, CreateOption)]
pub enum TopOldManiaVersion {
    #[option(name = "March 2014 - May 2018", value = "march14_may18")]
    March14May18,
    #[option(name = "May 2018 - Now", value = "may18_now")]
    May18Now,
}

impl TryFrom<i32> for TopOldManiaVersion {
    type Error = &'static str;

    fn try_from(year: i32) -> Result<Self, Self::Error> {
        match year {
            i32::MIN..=2013 => Err("mania pp were not a thing until march 2014. \
                I think? Don't quote me on that :^)"),
            2014..=2019 => Ok(Self::March14May18),
            _ => Ok(Self::May18Now),
        }
    }
}

pub async fn slash_topold(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let args = TopOld::from_interaction(command.input_data())?;

    topold(ctx, command.into(), args).await
}

#[command]
#[desc("Display a user's top plays on different pp versions")]
#[help(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    Note that the command will **not** change scores, just recalculate their pp.\n\
    The osu!standard pp history looks roughly like this:\n\
    - 2012: ppv1 (unavailable)\n\
    - 2014: ppv2 (unavailable)\n\
    - 2015: High CS nerf(?)\n\
    - 2018: [HD adjustment](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)\n\
    - 2019: [Angles, speed, spaced streams](https://osu.ppy.sh/home/news/2019-02-05-new-changes-to-star-rating-performance-points)\n\
    - 2021: [High AR nerf, NF & SO buff, speed & acc adjustment](https://osu.ppy.sh/home/news/2021-01-14-performance-points-updates)\n\
    - 2021: [Diff spike nerf, AR buff, FL-AR adjust](https://osu.ppy.sh/home/news/2021-07-27-performance-points-star-rating-updates)\n\
    - 2021: [Rhythm buff, slider buff, FL skill](https://osu.ppy.sh/home/news/2021-11-09-performance-points-star-rating-updates)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2015")]
#[alias("to")]
#[group(Osu)]
async fn prefix_topold(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopOld::args(GameMode::STD, args) {
        Ok(args) => topold(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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
    - 2014: ppv1\n\
    - 2018: [ppv2](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2015")]
#[alias("tom")]
#[group(Mania)]
async fn prefix_topoldmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopOld::args(GameMode::MNA, args) {
        Ok(args) => topold(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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
    - 2014: ppv1\n\
    - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-09-15-changes-to-osutaiko-star-rating)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2015")]
#[alias("tot")]
#[group(Taiko)]
async fn prefix_topoldtaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopOld::args(GameMode::TKO, args) {
        Ok(args) => topold(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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
    - 2014: ppv1\n\
    - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-05-14-osucatch-scoring-updates)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2019")]
#[alias("toc")]
#[group(Catch)]
async fn prefix_topoldctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match TopOld::args(GameMode::CTB, args) {
        Ok(args) => topold(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

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

        let year = year.unwrap_or_else(|| Utc::now().year());

        let args = match mode {
            GameMode::STD => {
                let version = TopOldOsuVersion::try_from(year)?;

                let osu = TopOldOsu {
                    version,
                    name,
                    discord,
                };

                Self::Osu(osu)
            }
            GameMode::TKO => {
                let version = TopOldTaikoVersion::try_from(year)?;

                let taiko = TopOldTaiko {
                    version,
                    name,
                    discord,
                };

                Self::Taiko(taiko)
            }
            GameMode::CTB => {
                let version = TopOldCatchVersion::try_from(year)?;

                let catch = TopOldCatch {
                    version,
                    name,
                    discord,
                };

                Self::Catch(catch)
            }
            GameMode::MNA => {
                let version = TopOldManiaVersion::try_from(year)?;

                let mania = TopOldMania {
                    version,
                    name,
                    discord,
                };

                Self::Mania(mania)
            }
        };

        Ok(args)
    }

    fn date_range(&self) -> &'static str {
        match self {
            TopOld::Osu(o) => match o.version {
                TopOldOsuVersion::April15May18 => "between april 2015 and may 2018",
                TopOldOsuVersion::May18February19 => "between may 2018 and february 2019",
                TopOldOsuVersion::February19January21 => "between february 2019 and january 2021",
                TopOldOsuVersion::January21July21 => "between january 2021 and july 2021",
                TopOldOsuVersion::July21November21 => "between july 2021 and november 2021",
                TopOldOsuVersion::November21Now => "since november 2021",
            },
            TopOld::Taiko(t) => match t.version {
                TopOldTaikoVersion::March14September20 => "between march 2014 and september 2020",
                TopOldTaikoVersion::September20Now => "since september 2020",
            },
            TopOld::Catch(c) => match c.version {
                TopOldCatchVersion::March14May20 => "between march 2014 and may 2020",
                TopOldCatchVersion::May20Now => "since may 2020",
            },
            TopOld::Mania(m) => match m.version {
                TopOldManiaVersion::March14May18 => "between march 2014 and may 2018",
                TopOldManiaVersion::May18Now => "since may 2018",
            },
        }
    }
}

macro_rules! pp_std {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_result = $version::OsuPP::new(&$rosu_map).mods($mods).calculate();

        let max_pp = max_pp_result.pp();
        $score.map.as_mut().unwrap().stars = max_pp_result.stars() as f32;

        let pp_result = $version::OsuPP::new(&$rosu_map)
            .mods($mods)
            .attributes(PerformanceAttributes::from(max_pp_result))
            .n300($score.statistics.count_300 as usize)
            .n100($score.statistics.count_100 as usize)
            .n50($score.statistics.count_50 as usize)
            .misses($score.statistics.count_miss as usize)
            .combo($score.max_combo as usize)
            .calculate();

        $score.pp.replace(pp_result.pp() as f32);

        max_pp
    }};
}

macro_rules! pp_mna {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_result = $version::ManiaPP::new(&$rosu_map).mods($mods).calculate();

        let max_pp = max_pp_result.pp();
        $score.map.as_mut().unwrap().stars = max_pp_result.stars() as f32;

        let pp_result = $version::ManiaPP::new(&$rosu_map)
            .mods($mods)
            .attributes(PerformanceAttributes::from(max_pp_result))
            .score($score.score)
            .accuracy($score.accuracy)
            .calculate();

        $score.pp.replace(pp_result.pp() as f32);

        max_pp
    }};
}

macro_rules! pp_ctb {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_result = $version::FruitsPP::new(&$rosu_map).mods($mods).calculate();

        let max_pp = max_pp_result.pp();
        $score.map.as_mut().unwrap().stars = max_pp_result.stars() as f32;

        let pp_result = $version::FruitsPP::new(&$rosu_map)
            .mods($mods)
            .attributes(PerformanceAttributes::from(max_pp_result))
            .fruits($score.statistics.count_300 as usize)
            .droplets($score.statistics.count_100 as usize)
            .tiny_droplets($score.statistics.count_50 as usize)
            .tiny_droplet_misses($score.statistics.count_katu as usize)
            .misses($score.statistics.count_miss as usize)
            .combo($score.max_combo as usize)
            .calculate();

        $score.pp.replace(pp_result.pp() as f32);

        max_pp
    }};
}

macro_rules! pp_tko {
    ($version:ident, $rosu_map:ident, $score:ident, $mods:ident) => {{
        let max_pp_result = $version::TaikoPP::new(&$rosu_map).mods($mods).calculate();

        let max_pp = max_pp_result.pp();
        $score.map.as_mut().unwrap().stars = max_pp_result.stars() as f32;

        let pp_result = $version::TaikoPP::new(&$rosu_map)
            .mods($mods)
            .attributes(PerformanceAttributes::from(max_pp_result))
            .n300($score.statistics.count_300 as usize)
            .n100($score.statistics.count_100 as usize)
            .misses($score.statistics.count_miss as usize)
            .combo($score.max_combo as usize)
            .calculate();

        $score.pp.replace(pp_result.pp() as f32);

        max_pp
    }};
}

async fn topold(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: TopOld<'_>) -> BotResult<()> {
    let (name, mode) = match &args {
        TopOld::Osu(o) => (username_ref!(ctx, orig, o), GameMode::STD),
        TopOld::Taiko(t) => (username_ref!(ctx, orig, t), GameMode::TKO),
        TopOld::Catch(c) => (username_ref!(ctx, orig, c), GameMode::CTB),
        TopOld::Mania(m) => (username_ref!(ctx, orig, m), GameMode::MNA),
    };

    let name = match name {
        Some(name) => name,
        None => match ctx.psql().get_user_osu(orig.user_id()?).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), mode);
    let score_args = ScoreArgs::top(100).with_combo();

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
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

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    // Calculate bonus pp
    let actual_pp: f32 = scores
        .iter()
        .filter_map(|score| score.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - actual_pp;

    let mut scores_data = match modify_scores(&ctx, scores, &args).await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    // Sort by adjusted pp
    ScoreOrder::Pp.apply(&ctx, &mut scores_data).await;

    // Calculate adjusted pp
    let adjusted_pp: f32 = scores_data
        .iter()
        .map(|(i, Score { pp, .. }, ..)| pp.unwrap_or(0.0) * 0.95_f32.powi(*i as i32 - 1))
        .sum();

    let adjusted_pp = numbers::round((bonus_pp + adjusted_pp).max(0.0) as f32);

    // Accumulate all necessary data
    let content = format!(
        "`{name}`{plural} {mode}top100 {version}:",
        name = user.username,
        plural = plural(user.username.as_str()),
        mode = mode_str(mode),
        version = args.date_range(),
    );

    let pages = numbers::div_euclid(5, scores_data.len());
    let post_pp = user.statistics.as_ref().unwrap().pp;
    let iter = scores_data.iter().take(5);
    let embed_data_fut = TopIfEmbed::new(&user, iter, mode, adjusted_pp, post_pp, None, (1, pages));

    // Creating the embed
    let embed = embed_data_fut.await.build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = orig.create_message(&ctx, &builder).await?;

    // * Don't add maps of scores to DB since their stars were potentially changed

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = TopIfPagination::new(
        response,
        user,
        scores_data,
        mode,
        adjusted_pp,
        post_pp,
        None,
    );
    let owner = orig.user_id()?;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

async fn modify_scores(
    ctx: &Context,
    scores: Vec<Score>,
    args: &TopOld<'_>,
) -> BotResult<Vec<(usize, Score, Option<f32>)>> {
    let mut scores_data = Vec::with_capacity(scores.len());

    for (mut score, i) in scores.into_iter().zip(1..) {
        let map = score.map.as_ref().unwrap();

        if map.convert {
            scores_data.push((i, score, None));
            continue;
        }

        let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
        let rosu_map = Beatmap::from_path(map_path).await.map_err(PpError::from)?;
        let mods = score.mods.bits();

        let max_pp = match args {
            TopOld::Osu(o) => match o.version {
                TopOldOsuVersion::April15May18 => pp_std!(osu_2015, rosu_map, score, mods),
                TopOldOsuVersion::May18February19 => pp_std!(osu_2018, rosu_map, score, mods),
                TopOldOsuVersion::February19January21 => pp_std!(osu_2019, rosu_map, score, mods),
                TopOldOsuVersion::January21July21 => {
                    pp_std!(osu_2021_january, rosu_map, score, mods)
                }
                TopOldOsuVersion::July21November21 => pp_std!(osu_2021_july, rosu_map, score, mods),
                TopOldOsuVersion::November21Now => {
                    scores_data.push((i, score, Some(rosu_map.max_pp(mods).pp() as f32)));
                    continue;
                }
            },
            TopOld::Taiko(t) => match t.version {
                TopOldTaikoVersion::March14September20 => {
                    pp_tko!(taiko_ppv1, rosu_map, score, mods)
                }
                TopOldTaikoVersion::September20Now => {
                    scores_data.push((i, score, Some(rosu_map.max_pp(mods).pp() as f32)));
                    continue;
                }
            },
            TopOld::Catch(c) => match c.version {
                TopOldCatchVersion::March14May20 => pp_ctb!(fruits_ppv1, rosu_map, score, mods),
                TopOldCatchVersion::May20Now => {
                    scores_data.push((i, score, Some(rosu_map.max_pp(mods).pp() as f32)));
                    continue;
                }
            },
            TopOld::Mania(m) => match m.version {
                TopOldManiaVersion::March14May18 => pp_mna!(mania_ppv1, rosu_map, score, mods),
                TopOldManiaVersion::May18Now => {
                    scores_data.push((i, score, Some(rosu_map.max_pp(mods).pp() as f32)));
                    continue;
                }
            },
        };

        scores_data.push((i, score, Some(max_pp as f32)));
    }

    Ok(scores_data)
}

fn plural(name: &str) -> &'static str {
    match name.chars().last() {
        Some('s') => "'",
        Some(_) | None => "'s",
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "",
        GameMode::TKO => "taiko ",
        GameMode::CTB => "ctb ",
        GameMode::MNA => "mania ",
    }
}
