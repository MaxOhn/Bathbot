use std::{borrow::Cow, mem, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::ScoreSlim;
use bathbot_psql::model::configs::{GuildConfig, ScoreSize};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher, CowUtils, MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::{
    prelude::{
        GameMode, Grade, OsuError,
        RankStatus::{Approved, Loved, Qualified, Ranked},
        Score,
    },
    request::UserId,
};
use tokio::time::{sleep, Duration};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::{
        osu::{require_link, user_not_found},
        GameModeOption, GradeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    embeds::RecentEmbed,
    manager::{
        redis::osu::{UserArgs, UserArgsSlim},
        OsuMap,
    },
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt, MessageExt},
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
#[command]
#[desc("Display a user's most recent pass")]
#[help(
    "Display a user's most recent pass.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rp42 badewanne3` to get the 42nd most recent pass.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [grade=grade[..grade]]")]
#[examples("badewanne3", "grade=a", "whitecat grade=B")]
#[aliases("rp", "rps")]
#[group(Osu)]
async fn prefix_recentpass(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(None, args) {
        Ok(mut args) => {
            args.passes = Some(true);

            score(ctx, msg.into(), args).await
        }
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's most recent mania pass")]
#[help(
    "Display a user's most recent pass.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rpm42 badewanne3` to get the 42nd most recent score.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [grade=grade[..grade]]")]
#[examples("badewanne3", "grade=a", "whitecat grade=B")]
#[aliases("rpm")]
#[group(Mania)]
async fn prefix_recentpassmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Mania), args) {
        Ok(mut args) => {
            args.passes = Some(true);

            score(ctx, msg.into(), args).await
        }
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's most recent taiko pass")]
#[help(
    "Display a user's most recent pass.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rpt42 badewanne3` to get the 42nd most recent score.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [grade=grade[..grade]]")]
#[examples("badewanne3", "grade=a", "whitecat grade=B")]
#[alias("rpt")]
#[group(Taiko)]
async fn prefix_recentpasstaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Taiko), args) {
        Ok(mut args) => {
            args.passes = Some(true);

            score(ctx, msg.into(), args).await
        }
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("Display a user's most recent ctb pass")]
#[help(
    "Display a user's most recent pass.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rpc42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...`.\n\
    Available grades are `SS`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [grade=grade[..grade]]")]
#[examples("badewanne3", "grade=a", "whitecat grade=B")]
#[alias("rpc", "rpctb")]
#[group(Catch)]
async fn prefix_recentpassctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Catch), args) {
        Ok(mut args) => {
            args.passes = Some(true);

            score(ctx, msg.into(), args).await
        }
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

    let config = match ctx.user_config().with_osu_id(author).await {
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

    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
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

    // Retrieve the user and their recent scores
    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);

    let include_fails = match (grade, passes) {
        (_, Some(passes)) => !passes,
        (Some(Grade::F), _) | (None, None) => true,
        _ => false,
    };

    let scores_fut = ctx
        .osu_scores()
        .recent()
        .limit(100)
        .include_fails(include_fails)
        .exec_with_user(user_args);

    #[cfg(feature = "twitch")]
    let (scores_res, twitch_res) = tokio::join!(scores_fut, ctx.twitch().id_from_osu(&user_id));

    #[cfg(not(feature = "twitch"))]
    let scores_res = scores_fut.await;

    let (user, mut scores) = match scores_res {
        Ok((user, scores)) if scores.is_empty() => {
            let username = user.username();
            let content = format!(
                "No recent {}plays found for user `{username}`",
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
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user or scores");

            return Err(err);
        }
    };

    #[cfg(feature = "twitch")]
    let twitch_id = match twitch_res {
        Ok(id) => id,
        Err(err) => {
            warn!("{err:?}");

            None
        }
    };

    if let Some(grade) = grade {
        scores.retain(|score| score.grade.eq_letter(grade));
    } else if let Some(true) = passes {
        scores.retain(|score| score.grade != Grade::F);
    } else if let Some(false) = passes {
        scores.retain(|score| score.grade == Grade::F);
    }

    let num = index.unwrap_or(1).saturating_sub(1);

    let (score, map, tries) = {
        let len = scores.len();
        let mut iter = scores.into_iter().skip(num);

        let Some(score) = iter.next() else {
            let username = user.username();

            let content = format!(
                "There {verb} only {len} score{plural} in `{username}`'{genitive} recent history.",
                verb = if len != 1 { "are" } else { "is" },
                plural = if len != 1 { "s" } else { "" },
                genitive = if username.ends_with('s') { "" } else { "s" }
            );

            return orig.error(&ctx, content).await;
        };

        let map_id = score.map_id;
        let checksum = score.map.as_ref().and_then(|map| map.checksum.as_deref());

        let map = match ctx.osu_map().map(map_id, checksum).await {
            Ok(map) => map.convert(mode),
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(Report::new(err));
            }
        };

        let mods = score.mods;

        let tries = 1 + iter
            .take_while(|s| s.mods == mods && s.map_id == map_id)
            .count();

        (score, map, tries)
    };

    let user_id = user.user_id();
    let grade = score.grade;
    let status = map.status();
    let map_id = score.map_id;

    // Prepare retrieval of the the user's top 50 and score position on the map
    let map_score_fut = async {
        if grade != Grade::F && matches!(status, Ranked | Loved | Qualified | Approved) {
            let fut = ctx.osu().beatmap_user_score(map_id, user_id).mode(mode);

            Some(fut.await)
        } else {
            None
        }
    };

    let best_fut = async {
        if grade != Grade::F && status == Ranked {
            let user_args = UserArgsSlim::user_id(user_id).mode(mode);

            Some(ctx.osu_scores().top().limit(100).exec(user_args).await)
        } else {
            None
        }
    };

    #[cfg(feature = "twitch")]
    let twitch_fut = async {
        if let Some(user_id) = twitch_id {
            retrieve_vod(&ctx, user_id, &score, &map).await
        } else {
            None
        }
    };

    // Retrieve and parse response
    #[cfg(feature = "twitch")]
    let (map_score_res, best_res, twitch_vod) = tokio::join!(map_score_fut, best_fut, twitch_fut);
    #[cfg(not(feature = "twitch"))]
    let (map_score_res, best_res) = tokio::join!(map_score_fut, best_fut);

    let map_score = match map_score_res {
        None | Some(Err(OsuError::NotFound)) => None,
        Some(Ok(score)) => Some(score),
        Some(Err(err)) => {
            let err = Report::new(err).wrap_err("failed to get global scores");
            warn!("{err:?}");

            None
        }
    };

    #[allow(unused_mut)]
    let mut best = match best_res {
        None => None,
        Some(Ok(scores)) => Some(scores),
        Some(Err(err)) => {
            let err = Report::new(err).wrap_err("failed to get top scores");
            warn!("{err:?}");

            None
        }
    };

    let (guild_minimized_pp, guild_show_retries, guild_score_size) = match orig.guild_id() {
        Some(guild_id) => {
            let f = |config: &GuildConfig| {
                (config.minimized_pp, config.show_retries, config.score_size)
            };

            ctx.guild_config().peek(guild_id, f).await
        }
        None => (None, None, None),
    };

    let minimized_pp = config
        .minimized_pp
        .or(guild_minimized_pp)
        .unwrap_or_default();

    let entry = RecentEntry::new(&ctx, score, map).await;

    let data_fut = RecentEmbed::new(
        &user,
        &entry,
        best.as_deref(),
        map_score.as_ref(),
        #[cfg(feature = "twitch")]
        twitch_vod,
        minimized_pp,
        &ctx,
    );

    let embed_data = data_fut.await;

    // Creating the embed
    let show_retries = config.show_retries.or(guild_show_retries).unwrap_or(true);
    let score_size = config.score_size.or(guild_score_size).unwrap_or_default();

    let content = show_retries.then(|| format!("Try #{tries}"));

    // Only maximize if config allows it
    match score_size {
        ScoreSize::AlwaysMinimized => {
            let embed = embed_data.into_minimized();
            let mut builder = MessageBuilder::new().embed(embed);

            if let Some(content) = content {
                builder = builder.content(content);
            }

            orig.create_message(&ctx, &builder).await?;
        }
        ScoreSize::InitialMaximized => {
            let embed = embed_data.as_maximized();
            let mut builder = MessageBuilder::new().embed(embed);

            if let Some(content) = content {
                builder = builder.content(content);
            }

            let mut response = orig.create_message(&ctx, &builder).await?.model().await?;

            // Lacking permission to edit the message
            if !orig.can_view_channel() {
                return Ok(());
            }

            ctx.store_msg(response.id);
            let ctx = Arc::clone(&ctx);
            let permissions = orig.permissions();

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

                if let Some(update_fut) = response.update(&ctx, &builder, permissions) {
                    if let Err(err) = update_fut.await {
                        let report = Report::new(err).wrap_err("Failed to minimize embed");
                        warn!("{report:?}");
                    }
                }
            });
        }
        ScoreSize::AlwaysMaximized => {
            let embed = embed_data.as_maximized();
            let mut builder = MessageBuilder::new().embed(embed);

            if let Some(content) = content {
                builder = builder.content(content);
            }

            orig.create_message(&ctx, &builder).await?;
        }
    }

    Ok(())
}

#[cfg(feature = "twitch")]
async fn retrieve_vod(
    ctx: &Context,
    user_id: u64,
    score: &rosu_v2::prelude::Score,
    map: &crate::manager::OsuMap,
) -> Option<bathbot_model::TwitchVideo> {
    use std::fmt::Write;

    use rosu_pp::{beatmap::Break, Mods};

    match ctx.client().get_last_twitch_vod(user_id).await {
        Ok(Some(mut vod)) => {
            let parsed_map = &map.pp_map;

            let vod_start = vod.created_at.unix_timestamp();
            let vod_end = vod_start + vod.duration as i64;
            let mut map_len = map.seconds_drain() as f64;

            // Adjust map length with passed objects in case of fail
            if score.grade == Grade::F {
                let passed = score.total_hits() as f64;

                if map.mode() == GameMode::Catch {
                    // amount objects in .osu file != amount of hitobjects for catch
                    map_len += 2.0;
                } else if let Some(obj) = parsed_map.hit_objects.get(passed as usize - 1) {
                    // Get time of the last hitobject that was hit
                    // and then accumulate break time of all breaks
                    // up to that time
                    let break_time: f64 = parsed_map
                        .breaks
                        .iter()
                        .take_while(|b| b.end_time < obj.start_time)
                        .map(Break::duration)
                        .sum();

                    map_len = obj.start_time + (break_time / 1000.0);
                } else {
                    let total = map.n_objects() as f64;
                    map_len *= passed / total;

                    map_len += 2.0;
                }
            } else {
                map_len += parsed_map.total_break_time() / 1000.0;
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
    #[inline]
    fn from(args: Rs<'a>) -> Self {
        unsafe { mem::transmute(args) }
    }
}

async fn slash_rs(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Rs::from_interaction(command.input_data())?;

    score(ctx, (&mut command).into(), args.into()).await
}

pub struct RecentEntry {
    pub score: ScoreSlim,
    pub map: OsuMap,
    pub max_pp: f32,
    pub stars: f32,
}

impl RecentEntry {
    async fn new(ctx: &Context, score: Score, map: OsuMap) -> Self {
        let mut calc = ctx.pp(&map).mode(score.mode).mods(score.mods);
        let attrs = calc.performance().await;

        let max_pp = score
            .pp
            .filter(|_| score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania)
            .unwrap_or(attrs.pp() as f32);

        let pp = match score.pp {
            Some(pp) => pp,
            None => calc.score(&score).performance().await.pp() as f32,
        };

        Self {
            score: ScoreSlim::new(score, pp),
            map,
            stars: attrs.stars() as f32,
            max_pp,
        }
    }
}
