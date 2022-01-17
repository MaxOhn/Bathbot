use std::{cmp::Ordering, sync::Arc};

use chrono::{Datelike, Utc};
use eyre::Report;
use futures::stream::{FuturesUnordered, TryStreamExt};
use rosu_pp::{Beatmap, BeatmapExt, PerformanceAttributes};
use rosu_pp_older::*;
use rosu_v2::prelude::{GameMode, OsuError, Score};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    id::UserId,
};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        parse_discord, DoubleResultCow,
    },
    database::UserConfig,
    embeds::{EmbedData, TopIfEmbed},
    error::PPError,
    pagination::{Pagination, TopIfPagination},
    tracking::process_tracking,
    util::{
        constants::{
            common_literals::{CTB, DISCORD, MANIA, NAME, OSU, TAIKO},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        numbers,
        osu::prepare_beatmap_file,
        InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, MessageBuilder,
};

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

pub(super) async fn _topold(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: OldArgs,
) -> BotResult<()> {
    let OldArgs { config, version } = args;

    let mode = version
        .map(OldVersion::mode)
        .or(config.mode)
        .unwrap_or(GameMode::STD);

    let content = match (mode, version) {
        (GameMode::STD, None) => Some("osu! was not a thing until september 2007."),
        (GameMode::STD, Some(OldVersion::OsuRankedScore)) => {
            Some("Up until april 2012, ranked score was the skill metric.")
        }
        (GameMode::STD, Some(OldVersion::OsuPpV1)) => Some(
            "April 2012 till january 2014 was the reign of ppv1.\n\
            The source code is not available though \\:(",
        ),
        (GameMode::STD, Some(OldVersion::OsuPpV2)) => Some(
            "ppv2 replaced ppv1 in january 2014 and lasted until april 2015.\n\
            The source code is not available though \\:(",
        ),

        (GameMode::TKO, None) => {
            Some("taiko pp were not a thing until march 2014. I think? Don't quote me on that :^)")
        }

        (GameMode::CTB, None) => {
            Some("ctb pp were not a thing until march 2014. I think? Don't quote me on that :^)")
        }

        (GameMode::MNA, None) => {
            Some("mania pp were not a thing until march 2014. I think? Don't quote me on that :^)")
        }

        _ => None,
    };

    if let Some(content) = content {
        let builder = MessageBuilder::new().embed(content);
        data.create_message(&ctx, builder).await?;

        return Ok(());
    }

    let version = version.unwrap();

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), mode);
    let score_args = ScoreArgs::top(100).with_combo();

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return data.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_tracking(&ctx, &mut scores, Some(&user)).await;

    // Calculate bonus pp
    let actual_pp: f32 = scores
        .iter()
        .filter_map(|score| score.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - actual_pp;

    let scores_fut = scores
        .into_iter()
        .enumerate()
        .map(|(mut i, mut score)| async move {
            i += 1;
            let map = score.map.as_ref().unwrap();

            if map.convert {
                return Ok((i, score, None));
            }

            let map_path = prepare_beatmap_file(map.map_id).await?;
            let rosu_map = Beatmap::from_path(map_path).await.map_err(PPError::from)?;
            let mods = score.mods.bits();

            // Calculate pp values
            let max_pp = match version {
                OldVersion::OsuApr15May18 => pp_std!(osu_2015, rosu_map, score, mods),
                OldVersion::OsuMay18Feb19 => pp_std!(osu_2018, rosu_map, score, mods),
                OldVersion::OsuFeb19Jan21 => pp_std!(osu_2019, rosu_map, score, mods),
                OldVersion::OsuJan21Jul21 => pp_std!(osu_2021_january, rosu_map, score, mods),
                OldVersion::OsuJul21Nov21 => pp_std!(osu_2021_july, rosu_map, score, mods),
                OldVersion::ManiaMar14May18 => pp_mna!(mania_ppv1, rosu_map, score, mods),
                OldVersion::TaikoMar14Sep20 => pp_tko!(taiko_ppv1, rosu_map, score, mods),
                OldVersion::CatchMar14May20 => pp_ctb!(fruits_ppv1, rosu_map, score, mods),
                _ => return Ok((i, score, Some(rosu_map.max_pp(mods).pp() as f32))),
            };

            Ok((i, score, Some(max_pp as f32)))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect::<Vec<_>>();

    let mut scores_data = match scores_fut.await {
        Ok(scores) => scores,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Sort by adjusted pp
    scores_data.sort_unstable_by(|(_, s1, _), (_, s2, _)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

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
        version = version.date_range(),
    );

    let pages = numbers::div_euclid(5, scores_data.len());
    let post_pp = user.statistics.as_ref().unwrap().pp;
    let iter = scores_data.iter().take(5);
    let embed_data_fut = TopIfEmbed::new(&user, iter, mode, adjusted_pp, post_pp, (1, pages));

    // Creating the embed
    let embed = embed_data_fut.await.into_builder().build();
    let builder = MessageBuilder::new().content(content).embed(embed);
    let response_raw = data.create_message(&ctx, builder).await?;

    // * Don't add maps of scores to DB since their stars were potentially changed

    // Skip pagination if too few entries
    if scores_data.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = TopIfPagination::new(response, user, scores_data, mode, adjusted_pp, post_pp);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a user's top plays on different pp versions")]
#[long_desc(
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
#[example("badewanne3 2018", "\"freddie benson\" 2015")]
#[aliases("to")]
async fn topold(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match OldArgs::args(&ctx, &mut args, msg.author.id, GameMode::STD).await {
                Ok(Ok(old_args)) => {
                    _topold(ctx, CommandData::Message { msg, args, num }, old_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display a user's top mania plays on different pp versions")]
#[long_desc(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    Note that the command will **not** change scores, just recalculate their pp.\n\
    The osu!mania pp history looks roughly like this:\n\
    - 2014: ppv1\n\
    - 2018: [ppv2](https://osu.ppy.sh/home/news/2018-05-16-performance-updates)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2015")]
#[aliases("tom")]
async fn topoldmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match OldArgs::args(&ctx, &mut args, msg.author.id, GameMode::MNA).await {
                Ok(Ok(old_args)) => {
                    _topold(ctx, CommandData::Message { msg, args, num }, old_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display a user's top taiko plays on different pp versions")]
#[long_desc(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    Note that the command will **not** change scores, just recalculate their pp.\n\
    The osu!taiko pp history looks roughly like this:\n\
    - 2014: ppv1\n\
    - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-09-15-changes-to-osutaiko-star-rating)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2015")]
#[aliases("tot")]
async fn topoldtaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match OldArgs::args(&ctx, &mut args, msg.author.id, GameMode::TKO).await {
                Ok(Ok(old_args)) => {
                    _topold(ctx, CommandData::Message { msg, args, num }, old_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display a user's top ctb plays on different pp versions")]
#[long_desc(
    "Display how the user's **current** top100 would have looked like \
    in a previous year.\n\
    Note that the command will **not** change scores, just recalculate their pp.\n\
    The osu!ctb pp history looks roughly like this:\n\
    - 2014: ppv1\n\
    - 2020: [Revamp](https://osu.ppy.sh/home/news/2020-05-14-osucatch-scoring-updates)"
)]
#[usage("[username] [year]")]
#[example("\"freddie benson\" 2019")]
#[aliases("toc")]
async fn topoldctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match OldArgs::args(&ctx, &mut args, msg.author.id, GameMode::CTB).await {
                Ok(Ok(old_args)) => {
                    _topold(ctx, CommandData::Message { msg, args, num }, old_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_top(ctx, *command).await,
    }
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

#[derive(Copy, Clone)]
pub(super) enum OldVersion {
    OsuRankedScore,
    OsuPpV1,
    OsuPpV2,
    OsuApr15May18,
    OsuMay18Feb19,
    OsuFeb19Jan21,
    OsuJan21Jul21,
    OsuJul21Nov21,
    OsuNov21,
    ManiaMar14May18,
    ManiaMay18,
    TaikoMar14Sep20,
    TaikoSep20,
    CatchMar14May20,
    CatchMay20,
}

impl OldVersion {
    fn get(mode: GameMode, year: u32) -> Option<Self> {
        match mode {
            GameMode::STD => match year {
                0..=2006 => None,
                2007..=2011 => Some(Self::OsuRankedScore),
                2012..=2013 => Some(Self::OsuPpV1),
                2014 => Some(Self::OsuPpV2),
                2015..=2017 => Some(Self::OsuApr15May18),
                2018 => Some(Self::OsuMay18Feb19),
                2019..=2020 => Some(Self::OsuFeb19Jan21),
                2021 => Some(Self::OsuJan21Jul21),
                _ => Some(Self::OsuNov21),
            },
            GameMode::TKO => match year {
                0..=2013 => None,
                2014..=2019 => Some(Self::TaikoMar14Sep20),
                _ => Some(Self::TaikoSep20),
            },
            GameMode::CTB => match year {
                0..=2013 => None,
                2014..=2019 => Some(Self::CatchMar14May20),
                _ => Some(Self::CatchMay20),
            },
            GameMode::MNA => match year {
                0..=2013 => None,
                2014..=2019 => Some(Self::ManiaMar14May18),
                _ => Some(Self::ManiaMay18),
            },
        }
    }

    fn date_range(self) -> &'static str {
        match self {
            OldVersion::OsuRankedScore => "between 2007 and april 2012",
            OldVersion::OsuPpV1 => "between april 2012 and january 2014",
            OldVersion::OsuPpV2 => "between january 2014 and april 2015",
            OldVersion::OsuApr15May18 => "between april 2015 and may 2018",
            OldVersion::OsuMay18Feb19 => "between may 2018 and february 2019",
            OldVersion::OsuFeb19Jan21 => "between february 2019 and january 2021",
            OldVersion::OsuJan21Jul21 => "between january 2021 and july 2021",
            OldVersion::OsuJul21Nov21 => "between july 2021 and november 2021",
            OldVersion::OsuNov21 => "since november 2021",

            OldVersion::ManiaMar14May18 => "between march 2014 and may 2018",
            OldVersion::ManiaMay18 => "since may 2018",

            OldVersion::TaikoMar14Sep20 => "between march 2014 and september 2020",
            OldVersion::TaikoSep20 => "since september 2020",

            OldVersion::CatchMar14May20 => "between march 2014 and may 2020",
            OldVersion::CatchMay20 => "since may 2020",
        }
    }

    fn mode(self) -> GameMode {
        match self {
            OldVersion::OsuRankedScore
            | OldVersion::OsuPpV1
            | OldVersion::OsuPpV2
            | OldVersion::OsuApr15May18
            | OldVersion::OsuMay18Feb19
            | OldVersion::OsuFeb19Jan21
            | OldVersion::OsuJan21Jul21
            | OldVersion::OsuJul21Nov21
            | OldVersion::OsuNov21 => GameMode::STD,
            OldVersion::ManiaMar14May18 | OldVersion::ManiaMay18 => GameMode::MNA,
            OldVersion::TaikoMar14Sep20 | OldVersion::TaikoSep20 => GameMode::TKO,
            OldVersion::CatchMar14May20 | OldVersion::CatchMay20 => GameMode::CTB,
        }
    }
}

pub(super) struct OldArgs {
    config: UserConfig,
    version: Option<OldVersion>,
}

impl OldArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
        mode: GameMode,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;

        let first = args.next();
        let second = args.next();

        const ERR_PARSE_YEAR: &str = "Failed to parse year. Be sure to specify a valid number.";

        let (name, year) = match second {
            Some(second) => match second.parse() {
                Ok(num) => (first, num),
                Err(_) => return Ok(Err(ERR_PARSE_YEAR.into())),
            },
            None => match first {
                Some(first) => match first.parse() {
                    Ok(num) => (None, num),
                    Err(_) => (Some(first), Utc::now().year() as u32),
                },
                None => (None, Utc::now().year() as u32),
            },
        };

        if let Some(name) = name {
            match check_user_mention(ctx, name).await? {
                Ok(osu) => config.osu = Some(osu),
                Err(content) => return Ok(Err(content)),
            }
        }

        if mode == GameMode::STD {
            config.mode.get_or_insert(GameMode::STD);
        } else {
            config.mode = Some(mode);
        }

        let version = OldVersion::get(mode, year);

        Ok(Ok(Self { config, version }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        mut options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let option = options.pop().ok_or(Error::InvalidCommandOptions)?;

        let version = match option.value {
            CommandOptionValue::SubCommand(options) => {
                let mut version = None;

                for option in options {
                    match option.value {
                        CommandOptionValue::String(value) => match option.name.as_str() {
                            NAME => config.osu = Some(value.into()),
                            "version" => version = Some(value),
                            _ => return Err(Error::InvalidCommandOptions),
                        },
                        CommandOptionValue::User(value) => match option.name.as_str() {
                            DISCORD => match parse_discord(ctx, value).await? {
                                Ok(osu) => config.osu = Some(osu),
                                Err(content) => return Ok(Err(content)),
                            },
                            _ => return Err(Error::InvalidCommandOptions),
                        },
                        _ => return Err(Error::InvalidCommandOptions),
                    }
                }

                let version = version.ok_or(Error::InvalidCommandOptions)?;

                match option.name.as_str() {
                    OSU => match version.as_str() {
                        "april15_may18" => Some(OldVersion::OsuApr15May18),
                        "may18_february19" => Some(OldVersion::OsuMay18Feb19),
                        "february19_january21" => Some(OldVersion::OsuFeb19Jan21),
                        "january21_july21" => Some(OldVersion::OsuJan21Jul21),
                        "july21_november21" => Some(OldVersion::OsuJul21Nov21),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    TAIKO => match version.as_str() {
                        "march14_september20" => Some(OldVersion::TaikoMar14Sep20),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    CTB => match version.as_str() {
                        "march14_may20" => Some(OldVersion::CatchMar14May20),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    MANIA => match version.as_str() {
                        "march14_may18" => Some(OldVersion::ManiaMar14May18),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                }
            }
            _ => return Err(Error::InvalidCommandOptions),
        };

        Ok(Ok(Self { config, version }))
    }
}
