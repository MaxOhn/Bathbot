use std::{borrow::Cow, mem, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::ScoreSlim;
use bathbot_psql::model::configs::{GuildConfig, MinimizedPp, Retries, ScoreSize};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher, CowUtils, MessageOrigin,
};
use eyre::{Report, Result};
use rand::{thread_rng, Rng};
use rosu_v2::{
    prelude::{
        GameMod, GameMode, GameMods, Grade, OsuError,
        RankStatus::{Approved, Loved, Qualified, Ranked},
        Score,
    },
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    guild::Permissions,
    id::{marker::UserMarker, Id},
};

use super::RecentScore;
use crate::{
    active::{impls::RecentScoreEdit, ActiveMessages},
    commands::{
        osu::{require_link, user_not_found},
        GameModeOption, GradeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    manager::{
        redis::osu::{UserArgs, UserArgsSlim},
        OsuMap, OwnedReplayScore,
    },
    util::{interaction::InteractionCommand, ChannelExt, CheckPermissions, InteractionCommandExt},
    Context,
};

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
            index: num.to_string_opt().map(Cow::Owned),
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

    let user_config_fut = ctx.user_config().with_osu_id(author);
    let guild_values_fut = get_guild_values(&ctx, &orig);

    let (user_config_res, guild_values) = tokio::join!(user_config_fut, guild_values_fut);

    let config = match user_config_res {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to get user config"));
        }
    };

    let GuildValues {
        minimized_pp: guild_minimized_pp,
        retries: guild_retries,
        score_size: guild_score_size,
        render_button: guild_render_button,
    } = guild_values;

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
        (Some(Grade::F), Some(true)) => return orig.error(&ctx, ":clown:").await,
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
        if let Grade::F = grade {
            scores.retain(|score| !score.passed);
        } else {
            scores.retain(|score| score.grade.eq_letter(grade));
        }
    } else if let Some(passed) = passes {
        scores.retain(|score| passed == score.passed);
    }

    let num = match index.as_deref() {
        Some("random" | "?") => match scores.is_empty() {
            false => thread_rng().gen_range(0..scores.len()),
            true => 0,
        },
        Some(n) => match n.parse::<usize>() {
            Ok(n) => n.saturating_sub(1),
            Err(_) => {
                let content = "Failed to parse index. \
                Must be an integer between 1 and 100 or `random` / `?`.";

                return orig.error(&ctx, content).await;
            }
        },
        None => 0,
    };

    let retries = config
        .retries
        .or(guild_retries)
        .unwrap_or(Retries::ConsiderMods);

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

        let mods = &score.mods;

        let tries = match retries {
            Retries::Hide => None,
            Retries::ConsiderMods => {
                fn same_mods(a: &GameMods, b: &GameMods) -> bool {
                    a.iter().zip(b.iter()).all(|(a, b)| match (a, b) {
                        (GameMod::DoubleTimeOsu(a), GameMod::NightcoreOsu(b))
                        | (GameMod::NightcoreOsu(b), GameMod::DoubleTimeOsu(a)) => {
                            a.speed_change.eq(&b.speed_change)
                        }
                        (GameMod::SuddenDeathOsu(a), GameMod::PerfectOsu(b))
                        | (GameMod::PerfectOsu(b), GameMod::SuddenDeathOsu(a)) => {
                            a.restart.eq(&b.restart)
                        }
                        (GameMod::DoubleTimeTaiko(a), GameMod::NightcoreTaiko(b))
                        | (GameMod::NightcoreTaiko(b), GameMod::DoubleTimeTaiko(a)) => {
                            a.speed_change.eq(&b.speed_change)
                        }
                        (GameMod::SuddenDeathTaiko(a), GameMod::PerfectTaiko(b))
                        | (GameMod::PerfectTaiko(b), GameMod::SuddenDeathTaiko(a)) => {
                            a.restart.eq(&b.restart)
                        }
                        (GameMod::DoubleTimeCatch(a), GameMod::NightcoreCatch(b))
                        | (GameMod::NightcoreCatch(b), GameMod::DoubleTimeCatch(a)) => {
                            a.speed_change.eq(&b.speed_change)
                        }
                        (GameMod::SuddenDeathCatch(a), GameMod::PerfectCatch(b))
                        | (GameMod::PerfectCatch(b), GameMod::SuddenDeathCatch(a)) => {
                            a.restart.eq(&b.restart)
                        }
                        (GameMod::DoubleTimeMania(a), GameMod::NightcoreMania(b))
                        | (GameMod::NightcoreMania(b), GameMod::DoubleTimeMania(a)) => {
                            a.speed_change.eq(&b.speed_change)
                        }
                        (GameMod::SuddenDeathMania(a), GameMod::PerfectMania(b))
                        | (GameMod::PerfectMania(b), GameMod::SuddenDeathMania(a)) => {
                            a.restart.eq(&b.restart)
                        }
                        (a, b) => a.eq(b),
                    })
                }

                Some(
                    1 + iter
                        .take_while(|s| same_mods(&s.mods, mods) && s.map_id == map_id)
                        .count(),
                )
            }
            Retries::IgnoreMods => Some(1 + iter.take_while(|s| s.map_id == map_id).count()),
        };

        (score, map, tries)
    };

    let user_id = user.user_id();
    let grade = if score.passed { score.grade } else { Grade::F };
    let status = map.status();
    let map_id = score.map_id;
    let score_id = score.legacy_score_id;

    let mut with_miss_analyzer = orig
        .guild_id()
        .map_or(false, |guild| ctx.has_miss_analyzer(&guild));

    let mut with_render = match (guild_render_button, config.render_button) {
        (None | Some(true), None) => true,
        (None | Some(true), Some(with_render)) => with_render,
        (Some(false), _) => false,
    };

    // Prepare retrieval of the the user's top 50 and score position on the map
    let map_score_fut = async {
        if grade != Grade::F && matches!(status, Ranked | Loved | Qualified | Approved) {
            let fut = ctx.osu().beatmap_user_score(map_id, user_id).mode(mode);

            Some(fut.await)
        } else {
            None
        }
    };

    let top100_fut = async {
        if grade != Grade::F {
            let user_args = UserArgsSlim::user_id(user_id).mode(mode);

            Some(ctx.osu_scores().top().limit(100).exec(user_args).await)
        } else {
            None
        }
    };

    let score_id_opt = score_id.zip(orig.guild_id());
    with_miss_analyzer &= mode == GameMode::Osu;
    with_render &= mode == GameMode::Osu
        && score.replay
        && orig.has_permission_to(Permissions::SEND_MESSAGES)
        && ctx.ordr().is_some();

    let miss_analyzer_fut = async {
        if let Some((score_id, guild_id)) = score_id_opt.filter(|_| with_miss_analyzer) {
            debug!(score_id, "Sending score id to miss analyzer");

            ctx.client()
                .miss_analyzer_score_request(guild_id.get(), score_id)
                .await
        } else {
            Ok(false)
        }
    };

    #[cfg(feature = "twitch")]
    let twitch_fut = async {
        if let Some(user_id) = twitch_id {
            twitch_stream(&ctx, user_id, &score, &map).await
        } else {
            None
        }
    };

    #[cfg(feature = "twitch")]
    let (map_score_res, top100_res, miss_analyzer_res, twitch_stream) =
        tokio::join!(map_score_fut, top100_fut, miss_analyzer_fut, twitch_fut);

    #[cfg(not(feature = "twitch"))]
    let (map_score_res, top100_res, miss_analyzer_res) =
        tokio::join!(map_score_fut, top100_fut, miss_analyzer_fut);

    let map_score = match map_score_res {
        None | Some(Err(OsuError::NotFound)) => None,
        Some(Ok(score)) => Some(score),
        Some(Err(err)) => {
            warn!(?err, "Failed to get global scores");

            None
        }
    };

    let top100 = match top100_res {
        Some(Ok(scores)) => Some(scores),
        None => None,
        Some(Err(err)) => {
            warn!(?err, "Failed to get top100");

            None
        }
    };

    match miss_analyzer_res {
        Ok(wants_button) => with_miss_analyzer &= wants_button,
        Err(err) => {
            warn!(?err, "Failed to send score id to miss analyzer");
            with_miss_analyzer = false;
        }
    }

    let minimized_pp = config
        .minimized_pp
        .or(guild_minimized_pp)
        .unwrap_or_default();

    let replay_score = with_render
        .then(|| OwnedReplayScore::from_score(&score))
        .flatten();

    let entry = RecentEntry::new(&ctx, score, map).await;
    let origin = MessageOrigin::new(orig.guild_id(), orig.channel_id());

    let score_size = config.score_size.or(guild_score_size).unwrap_or_default();
    let content = tries.map(|tries| format!("Try #{tries}"));

    let active_msg_fut = RecentScoreEdit::create(
        &ctx,
        &user,
        &entry,
        top100.as_deref(),
        map_score.as_ref(),
        #[cfg(feature = "twitch")]
        twitch_stream,
        minimized_pp,
        score_id,
        with_miss_analyzer,
        replay_score,
        &origin,
        score_size,
        content,
    );

    ActiveMessages::builder(active_msg_fut.await)
        .start_by_update(true)
        .begin(ctx, orig)
        .await
}

#[cfg(feature = "twitch")]
pub enum RecentTwitchStream {
    Stream {
        login: Box<str>,
    },
    Video {
        username: Box<str>,
        login: Box<str>,
        vod_url: Box<str>,
    },
}

#[cfg(feature = "twitch")]
impl RecentTwitchStream {
    fn new_stream(login: Box<str>) -> Self {
        Self::Stream { login }
    }

    fn new_vod(username: Box<str>, login: Box<str>, vod_url: String) -> Self {
        Self::Video {
            username,
            login,
            vod_url: vod_url.into_boxed_str(),
        }
    }
}

#[cfg(feature = "twitch")]
async fn twitch_stream(
    ctx: &Context,
    user_id: u64,
    score: &rosu_v2::prelude::Score,
    map: &crate::manager::OsuMap,
) -> Option<RecentTwitchStream> {
    let is_online = ctx.online_twitch_streams().is_user_online(user_id);

    if is_online {
        match ctx.client().get_last_twitch_vod(user_id).await {
            Ok(Some(vod)) => {
                let score_started_at = score_started_at(score, map);

                let vod_start = vod.created_at;
                let vod_end = vod.ended_at();

                if vod_start < score_started_at && score_started_at < vod_end {
                    let mut url = vod.url;
                    let offset = score_started_at - vod_start;
                    bathbot_model::TwitchVideo::append_url_timestamp(&mut url, offset);

                    return Some(RecentTwitchStream::new_vod(vod.username, vod.login, url));
                }
            }
            Ok(None) => {}
            Err(err) => {
                warn!(?err, "Failed to get twitch vod");
                ctx.online_twitch_streams().set_offline_by_user(user_id);

                return None;
            }
        }

        match ctx.client().get_twitch_stream(user_id).await {
            Ok(Some(stream)) => {
                if stream.live {
                    Some(RecentTwitchStream::new_stream(stream.login))
                } else {
                    let guard = ctx.online_twitch_streams().guard();
                    ctx.online_twitch_streams().set_offline(&stream, &guard);

                    None
                }
            }
            Ok(None) => {
                // TODO: remove twitch id from user config

                None
            }
            Err(err) => {
                warn!(?err, "Failed to get twitch stream");
                ctx.online_twitch_streams().set_offline_by_user(user_id);

                None
            }
        }
    } else {
        match ctx.client().get_twitch_stream(user_id).await {
            Ok(Some(stream)) => {
                if !stream.live {
                    return None;
                }

                {
                    let guard = ctx.online_twitch_streams().guard();
                    ctx.online_twitch_streams().set_online(&stream, &guard);
                }

                match ctx.client().get_last_twitch_vod(user_id).await {
                    Ok(Some(vod)) => {
                        let score_started_at = score_started_at(score, map);

                        let vod_start = vod.created_at;
                        let vod_end = vod.ended_at();

                        if vod_start < score_started_at && score_started_at < vod_end {
                            let mut url = vod.url;
                            let offset = score_started_at - vod_start;
                            bathbot_model::TwitchVideo::append_url_timestamp(&mut url, offset);

                            Some(RecentTwitchStream::new_vod(vod.username, vod.login, url))
                        } else {
                            Some(RecentTwitchStream::new_stream(stream.login))
                        }
                    }
                    Ok(None) => Some(RecentTwitchStream::new_stream(stream.login)),
                    Err(err) => {
                        warn!(?err, "Failed to get twitch vod");

                        None
                    }
                }
            }
            Ok(None) => {
                // TODO: remove twitch id from user config

                None
            }
            Err(err) => {
                warn!(?err, "Failed to get twitch stream");

                None
            }
        }
    }
}

#[cfg(feature = "twitch")]
fn score_started_at(
    score: &rosu_v2::prelude::Score,
    map: &crate::manager::OsuMap,
) -> time::OffsetDateTime {
    let mut map_len = map.seconds_drain() as f64;

    // Adjust map length with passed objects in case of fail
    if score.passed {
        map_len += map.pp_map.total_break_time() / 1000.0;
    } else {
        let passed = score.total_hits();

        if map.mode() == GameMode::Catch {
            // amount objects in .osu file != amount of hitobjects for catch
            map_len += 2.0;
        } else if let Some(obj) = passed
            .checked_sub(1)
            .and_then(|i| map.pp_map.hit_objects.get(i as usize))
        {
            map_len = obj.start_time / 1000.0;
        } else {
            let total = map.n_objects();
            map_len *= passed as f64 / total as f64;

            map_len += 2.0;
        }
    }

    map_len /= rosu_pp::Mods::clock_rate(score.mods.bits());

    score.ended_at - std::time::Duration::from_secs(map_len as u64 + 3)
}

#[allow(unused)] // fields are used through transmute in From impl
#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "rs",
    desc = "Show a user's recent score",
    help = "Show a user's recent score.\n\
    To add a timestamp to a twitch VOD, be sure you linked yourself to a twitch account via `/config`."
)]
pub struct Rs<'a> {
    #[command(
        desc = "Specify a gamemode",
        help = "Specify a gamemode.\n\
        For mania the combo will be displayed as `[ combo / ratio ]` \
        with ratio being `n320/n300`."
    )]
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
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
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
    pub max_combo: u32,
    pub stars: f32,
}

impl RecentEntry {
    async fn new(ctx: &Context, score: Score, map: OsuMap) -> Self {
        let mut calc = ctx.pp(&map).mode(score.mode).mods(&score.mods);
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
            max_combo: attrs.max_combo() as u32,
        }
    }
}

#[derive(Default)]
struct GuildValues {
    minimized_pp: Option<MinimizedPp>,
    retries: Option<Retries>,
    score_size: Option<ScoreSize>,
    render_button: Option<bool>,
}

impl From<&GuildConfig> for GuildValues {
    fn from(config: &GuildConfig) -> Self {
        Self {
            minimized_pp: config.minimized_pp,
            retries: config.retries,
            score_size: config.score_size,
            render_button: config.render_button,
        }
    }
}

async fn get_guild_values(ctx: &Context, orig: &CommandOrigin<'_>) -> GuildValues {
    match orig.guild_id() {
        Some(guild_id) => {
            ctx.guild_config()
                .peek(guild_id, |config| GuildValues::from(config))
                .await
        }
        None => GuildValues::default(),
    }
}
