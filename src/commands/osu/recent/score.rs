use std::{borrow::Cow, fmt::Write, mem, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::{Report, Result};
use rosu_pp::{beatmap::Break, Mods};
use rosu_v2::prelude::{
    Beatmap, GameMode, Grade, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
    Score,
};
use tokio::time::{sleep, Duration};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::{
        osu::{get_user_and_scores, prepare_score, require_link, ScoreArgs, UserArgs},
        GameModeOption, GradeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    custom_client::TwitchVideo,
    database::{EmbedsSize, MinimizedPp},
    embeds::RecentEmbed,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        interaction::InteractionCommand,
        matcher,
        osu::prepare_beatmap_file,
        ChannelExt, CowUtils, InteractionCommandExt, MessageExt,
    },
    Context,
};

use super::RecentScore;

#[command]
#[desc("Display a user's most recent play")]
#[help(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `r42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[examples("badewanne3 pass=true", "grade=a", "whitecat grade=B")]
#[aliases("r", "rs")]
#[group(Osu)]
async fn prefix_recent(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(None, args) {
        Ok(args) => score(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's most recent mania play")]
#[help(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rm42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[examples("badewanne3 pass=true", "grade=a", "whitecat grade=B")]
#[aliases("rm")]
#[group(Mania)]
async fn prefix_recentmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Mania), args) {
        Ok(args) => score(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's most recent taiko play")]
#[help(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rt42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[examples("badewanne3 pass=true", "grade=a", "whitecat grade=B")]
#[alias("rt")]
#[group(Taiko)]
async fn prefix_recenttaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => score(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's most recent ctb play")]
#[help(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rc42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[examples("badewanne3 pass=true", "grade=a", "whitecat grade=B")]
#[alias("rc", "recentcatch")]
#[group(Catch)]
async fn prefix_recentctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Catch), args) {
        Ok(args) => score(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

impl<'m> RecentScore<'m> {
    fn args(mode: Option<GameModeOption>, args: Args<'m>) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut discord = None;
        let mut grade = None;
        let mut passes = None;
        let num = args.num;

        for arg in args.take(3).map(|arg| arg.cow_to_ascii_lowercase()) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "pass" | "p" | "passes" => match value {
                        "true" | "t" | "1" => passes = Some(true),
                        "false" | "f" | "0" => passes = Some(false),
                        _ => {
                            let content =
                                "Failed to parse `pass`. Must be either `true` or `false`.";

                            return Err(content.into());
                        }
                    },
                    "fail" | "fails" | "f" => match value {
                        "true" | "t" | "1" => passes = Some(false),
                        "false" | "f" | "0" => passes = Some(true),
                        _ => {
                            let content =
                                "Failed to parse `fail`. Must be either `true` or `false`.";

                            return Err(content.into());
                        }
                    },
                    "grade" | "g" => match value.parse::<GradeOption>() {
                        Ok(grade_) => grade = Some(grade_),
                        Err(content) => return Err(content.into()),
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\n\
                            Available options are: `grade` or `pass`."
                        );

                        return Err(content.into());
                    }
                }
            } else if let Some(id) = matcher::get_mention_user(&arg) {
                discord = Some(id);
            } else {
                name = Some(arg);
            }
        }

        if passes.is_some() {
            grade = None;
        }

        Ok(Self {
            mode,
            name,
            index: num.map(|n| n as usize),
            grade,
            passes,
            discord,
        })
    }
}

pub(super) async fn score(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RecentScore<'_>,
) -> Result<()> {
    let author = orig.user_id()?;

    let config = match ctx.user_config(author).await {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get user config"));
        }
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match config.username() {
            Some(name) => name.to_owned(),
            None => return require_link(&ctx, &orig).await,
        },
    };

    let RecentScore {
        grade,
        passes,
        index,
        ..
    } = args;

    let grade = grade.map(Grade::from);
    let mut twitch_id = None;

    // TODO: show twitch of given name if available
    if let Some(config_name) = config.username() {
        let config_name = config_name.cow_to_ascii_lowercase();
        let name = name.cow_to_ascii_lowercase();

        if config_name == name {
            twitch_id = Some(config.twitch_id);
        }
    }

    // Retrieve the user and their recent scores
    let user_args = UserArgs::new(&name, mode);

    let include_fails = match (grade, passes) {
        (_, Some(passes)) => !passes,
        (Some(Grade::F), _) | (None, None) => true,
        _ => false,
    };

    let score_args = ScoreArgs::recent(100).include_fails(include_fails);

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((_, scores)) if scores.is_empty() => {
            let content = format!(
                "No recent {}plays found for user `{name}`",
                match mode {
                    GameMode::Osu => "",
                    GameMode::Taiko => "taiko ",
                    GameMode::Catch => "ctb ",
                    GameMode::Mania => "mania ",
                },
            );

            return orig.error(&ctx, content).await;
        }
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");
            orig.error(&ctx, content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user or scores");

            return Err(report);
        }
    };

    // Overwrite default mode
    user.mode = mode;

    if let Some(grade) = grade {
        scores.retain(|score| score.grade.eq_letter(grade));
    } else if let Some(true) = passes {
        scores.retain(|score| score.grade != Grade::F);
    } else if let Some(false) = passes {
        scores.retain(|score| score.grade == Grade::F);
    }

    let num = index.unwrap_or(1).saturating_sub(1);

    let (score, tries) = {
        let mut iter = scores.iter_mut().skip(num);

        match iter.next() {
            Some(score) => match prepare_score(&ctx, score).await {
                Ok(_) => {
                    let mods = score.mods;
                    let map_id = map_id!(score).unwrap();

                    let tries = 1 + iter
                        .take_while(|s| map_id!(s).unwrap() == map_id && s.mods == mods)
                        .count();

                    (score, tries)
                }
                Err(err) => {
                    let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                    let report = Report::new(err).wrap_err("failed to prepare score");

                    return Err(report);
                }
            },
            None => {
                let content = format!(
                    "There {verb} only {num} score{plural} in `{name}`'{genitive} recent history.",
                    verb = if scores.len() != 1 { "are" } else { "is" },
                    num = scores.len(),
                    plural = if scores.len() != 1 { "s" } else { "" },
                    name = name,
                    genitive = if name.ends_with('s') { "" } else { "s" }
                );

                return orig.error(&ctx, content).await;
            }
        }
    };

    let map = score.map.as_ref().unwrap();

    // Prepare retrieval of the the user's top 50 and score position on the map
    let map_score_fut = async {
        if score.grade != Grade::F && matches!(map.status, Ranked | Loved | Qualified | Approved) {
            let fut = ctx
                .osu()
                .beatmap_user_score(map.map_id, user.user_id)
                .mode(mode);

            Some(fut.await)
        } else {
            None
        }
    };

    let best_fut = async {
        if score.grade != Grade::F && map.status == Ranked {
            let fut = ctx
                .osu()
                .user_scores(user.user_id)
                .best()
                .limit(100)
                .mode(mode);

            Some(fut.await)
        } else {
            None
        }
    };

    let twitch_fut = async {
        let twitch_id = if let Some(id) = twitch_id {
            id
        } else {
            match ctx.psql().get_user_config_by_osu(&user.username).await {
                Ok(Some(config)) => config.twitch_id,
                Ok(None) => None,
                Err(err) => {
                    warn!("{:?}", err.wrap_err("Failed to get config of input name"));

                    None
                }
            }
        };

        if let Some(user_id) = twitch_id {
            retrieve_vod(&ctx, user_id, &*score, map).await
        } else {
            None
        }
    };

    // Retrieve and parse response
    let (map_score_result, best_result, twitch_vod) =
        tokio::join!(map_score_fut, best_fut, twitch_fut);

    let map_score = match map_score_result {
        None | Some(Err(OsuError::NotFound)) => None,
        Some(Ok(score)) => Some(score),
        Some(Err(err)) => {
            let report = Report::new(err).wrap_err("Failed to get global scores");
            warn!("{report:?}");

            None
        }
    };

    #[allow(unused_mut)]
    let mut best = match best_result {
        None => None,
        Some(Ok(scores)) => Some(scores),
        Some(Err(err)) => {
            let report = Report::new(err).wrap_err("Failed to get top scores");
            warn!("{report:?}");

            None
        }
    };

    let guild_id = orig.guild_id();

    let minimized_pp = match (config.minimized_pp, guild_id) {
        (Some(pp), _) => pp,
        (None, Some(guild)) => ctx.guild_minimized_pp(guild).await,
        (None, None) => MinimizedPp::default(),
    };

    let data_fut = RecentEmbed::new(
        &user,
        score,
        best.as_deref(),
        map_score.as_ref(),
        twitch_vod,
        minimized_pp,
        &ctx,
    );

    let embed_data = match data_fut.await {
        Ok(data) => data,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to create embed"));
        }
    };

    // Creating the embed
    let show_retries = match (config.show_retries, guild_id) {
        (Some(show_retries), _) => show_retries,
        (None, Some(guild)) => ctx.guild_show_retries(guild).await,
        (None, None) => true,
    };

    let content = show_retries.then(|| format!("Try #{tries}"));

    let embeds_size = match (config.score_size, guild_id) {
        (Some(size), _) => size,
        (None, Some(guild)) => ctx.guild_embeds_maximized(guild).await,
        (None, None) => EmbedsSize::default(),
    };

    // Only maximize if config allows it
    match embeds_size {
        EmbedsSize::AlwaysMinimized => {
            let embed = embed_data.into_minimized();
            let mut builder = MessageBuilder::new().embed(embed);

            if let Some(content) = content {
                builder = builder.content(content);
            }

            orig.create_message(&ctx, &builder).await?;
        }
        EmbedsSize::InitialMaximized => {
            let embed = embed_data.as_maximized();
            let mut builder = MessageBuilder::new().embed(embed);

            if let Some(content) = content {
                builder = builder.content(content);
            }

            let mut response = orig.create_message(&ctx, &builder).await?.model().await?;
            ctx.store_msg(response.id);
            let ctx = Arc::clone(&ctx);

            // Wait for minimizing
            tokio::spawn(async move {
                sleep(Duration::from_secs(45)).await;

                if !ctx.remove_msg(response.id) {
                    return;
                }

                let embed = embed_data.into_minimized();
                let mut builder = MessageBuilder::new().embed(embed);

                if !response.content.is_empty() {
                    builder = builder.content(mem::take(&mut response.content));
                }

                if let Err(err) = response.update(&ctx, &builder).await {
                    let report = Report::new(err).wrap_err("Failed to minimize embed");
                    warn!("{report:?}");
                }
            });
        }
        EmbedsSize::AlwaysMaximized => {
            let embed = embed_data.as_maximized();
            let mut builder = MessageBuilder::new().embed(embed);

            if let Some(content) = content {
                builder = builder.content(content);
            }

            orig.create_message(&ctx, &builder).await?;
        }
    }

    // Set map on garbage collection list if unranked
    ctx.map_garbage_collector(map).execute(&ctx);

    // Process user and their top scores for tracking
    #[cfg(feature = "osutracking")]
    if let Some(ref mut scores) = best {
        crate::tracking::process_osu_tracking(&ctx, scores, Some(&user)).await;
    }

    Ok(())
}

async fn retrieve_vod(
    ctx: &Context,
    user_id: u64,
    score: &Score,
    map: &Beatmap,
) -> Option<TwitchVideo> {
    match ctx.client().get_last_twitch_vod(user_id).await {
        Ok(Some(mut vod)) => {
            // Parse map to get data about breaks
            let parsed_map = match prepare_beatmap_file(ctx, map.map_id).await {
                Ok(path) => match rosu_pp::Beatmap::from_path(path).await {
                    Ok(map) => Some(map),
                    Err(err) => {
                        let report = Report::new(err).wrap_err("Failed to parse map");
                        warn!("{report:?}");

                        None
                    }
                },
                Err(err) => {
                    warn!("{:?}", err.wrap_err("Failed to prepare map"));

                    None
                }
            };

            let vod_start = vod.created_at.unix_timestamp();
            let vod_end = vod_start + vod.duration as i64;
            let mut map_len = map.seconds_drain as f64;

            // Adjust map length with passed objects in case of fail
            if score.grade == Grade::F {
                let passed = score.total_hits() as f64;

                if map.mode == GameMode::Catch {
                    // amount objects in .osu file != amount of hitobjects for catch
                    map_len += 2.0;
                } else if let Some(map) = parsed_map {
                    // Get time of the last hitobject that was hit
                    // and then accumulate break time of all breaks
                    // up to that time
                    let obj = &map.hit_objects[passed as usize - 1];

                    let break_time: f64 = map
                        .breaks
                        .iter()
                        .take_while(|b| b.end_time < obj.start_time)
                        .map(Break::duration)
                        .sum();

                    map_len = obj.start_time + (break_time / 1000.0);
                } else {
                    let total = map.count_objects() as f64;
                    map_len *= passed / total;

                    map_len += 2.0;
                }
            } else if let Some(map) = parsed_map {
                map_len += map.total_break_time() / 1000.0;
            } else {
                map_len += 2.0;
            }

            map_len /= score.mods.bits().clock_rate();

            let map_start = score.ended_at.unix_timestamp() - map_len as i64 - 3;

            if vod_start > map_start || vod_end < map_start {
                return None;
            }

            let mut offset = map_start - vod_start;

            // Add timestamp
            vod.url.push_str("?t=");

            if offset >= 3600 {
                let _ = write!(vod.url, "{}h", offset / 3600);
                offset %= 3600;
            }

            if offset >= 60 {
                let _ = write!(vod.url, "{}m", offset / 60);
                offset %= 60;
            }

            if offset > 0 {
                let _ = write!(vod.url, "{offset}s");
            }

            Some(vod)
        }
        Ok(None) => None,
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to get twitch vod"));

            None
        }
    }
}

#[allow(unused)] // fields are used through transmute in From impl
#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "rs",
    help = "Show a user's recent score.\n\
    To add a timestamp to a twitch VOD, be sure you linked yourself to a twitch account via `/config`."
)]
/// Show a user's recent score
pub struct Rs<'a> {
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

impl<'a> From<Rs<'a>> for RecentScore<'a> {
    fn from(args: Rs<'a>) -> Self {
        unsafe { mem::transmute(args) }
    }
}

async fn slash_rs(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Rs::from_interaction(command.input_data())?;

    score(ctx, (&mut command).into(), args.into()).await
}
