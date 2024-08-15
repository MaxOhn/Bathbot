use std::{borrow::Cow, mem, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::{
    command_fields::{GameModeOption, GradeOption},
    embed_builder::SettingsImage,
};
use bathbot_psql::model::configs::{GuildConfig, Retries, ScoreData};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE},
    matcher, CowUtils, MessageOrigin,
};
use eyre::{Report, Result};
use rand::{thread_rng, Rng};
use rosu_v2::{
    prelude::{GameMod, GameMode, GameMods, Grade, OsuError, Score},
    request::UserId,
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    guild::Permissions,
    id::{marker::UserMarker, Id},
};

use super::RecentScore;
use crate::{
    active::{
        impls::{SingleScoreContent, SingleScorePagination},
        ActiveMessages,
    },
    commands::{
        osu::{map_strain_graph, require_link, user_not_found},
        utility::{MissAnalyzerCheck, ScoreEmbedDataWrap},
    },
    core::commands::{interaction::InteractionCommands, prefix::Args, CommandOrigin},
    manager::redis::osu::{UserArgs, UserArgsSlim},
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
async fn prefix_recent(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(None, args) {
        Ok(args) => score(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_recentmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Mania), args) {
        Ok(args) => score(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_recenttaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Taiko), args) {
        Ok(args) => score(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_recentctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Catch), args) {
        Ok(args) => score(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_recentpass(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(None, args) {
        Ok(mut args) => {
            args.passes = Some(true);

            score(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_recentpassmania(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Mania), args) {
        Ok(mut args) => {
            args.passes = Some(true);

            score(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_recentpasstaiko(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Taiko), args) {
        Ok(mut args) => {
            args.passes = Some(true);

            score(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

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
async fn prefix_recentpassctb(msg: &Message, args: Args<'_>) -> Result<()> {
    match RecentScore::args(Some(GameModeOption::Catch), args) {
        Ok(mut args) => {
            args.passes = Some(true);

            score(msg.into(), args).await
        }
        Err(content) => {
            msg.error(content).await?;

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

pub(super) async fn score(orig: CommandOrigin<'_>, args: RecentScore<'_>) -> Result<()> {
    let author = orig.user_id()?;

    let user_config_fut = Context::user_config().with_osu_id(author);
    let guild_values_fut = get_guild_values(&orig);

    let (user_config_res, guild_values) = tokio::join!(user_config_fut, guild_values_fut);

    let config = match user_config_res {
        Ok(config) => config,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to get user config"));
        }
    };

    let GuildValues {
        retries: guild_retries,
        render_button: guild_render_button,
        score_data: guild_score_data,
    } = guild_values;

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
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
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let include_fails = match (grade, passes) {
        (Some(Grade::F), Some(true)) => return orig.error(":clown:").await,
        (_, Some(passes)) => !passes,
        (Some(Grade::F), _) | (None, None) => true,
        _ => false,
    };

    let score_data = config.score_data.or(guild_score_data).unwrap_or_default();
    let legacy_scores = score_data.is_legacy();

    let scores_fut = Context::osu_scores()
        .recent(legacy_scores)
        .limit(100)
        .include_fails(include_fails)
        .exec_with_user(user_args);

    #[cfg(feature = "twitch")]
    let (scores_res, twitch_res) =
        tokio::join!(scores_fut, Context::twitch().id_from_osu(&user_id));

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

            return orig.error(content).await;
        }
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

                return orig.error(content).await;
            }
        },
        None => 0,
    };

    let retries = config
        .retries
        .or(guild_retries)
        .unwrap_or(Retries::ConsiderMods);

    let Some([score, prev_scores @ ..]) = scores.get(num..) else {
        let len = scores.len();
        let username = user.username();

        let content = format!(
            "There {verb} only {len} score{plural} in `{username}`'{genitive} recent history.",
            verb = if len != 1 { "are" } else { "is" },
            plural = if len != 1 { "s" } else { "" },
            genitive = if username.ends_with('s') { "" } else { "s" }
        );

        return orig.error(content).await;
    };

    let map_id = score.map_id;
    let mods = &score.mods;

    let tries = match retries {
        Retries::Hide => None,
        Retries::ConsiderMods => {
            // Treats DT & NC as well as SD & PF as the same.
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
                1 + prev_scores
                    .iter()
                    .take_while(|s| same_mods(&s.mods, mods) && s.map_id == map_id)
                    .count(),
            )
        }
        Retries::IgnoreMods => Some(
            1 + prev_scores
                .iter()
                .take_while(|s| s.map_id == map_id)
                .count(),
        ),
    };

    let user_id = user.user_id();
    let grade = if score.passed { score.grade } else { Grade::F };

    let mut with_miss_analyzer = orig
        .guild_id()
        .as_ref()
        .map_or(false, Context::has_miss_analyzer);

    let mut with_render = match (guild_render_button, config.render_button) {
        (None | Some(true), None) => true,
        (None | Some(true), Some(with_render)) => with_render,
        (Some(false), _) => false,
    };

    let (settings, missing_settings) = match config.score_embed {
        Some(settings) => (settings, false),
        None => (Default::default(), true),
    };

    let top100_fut = async {
        if grade != Grade::F || settings.buttons.pagination {
            let user_args = UserArgsSlim::user_id(user_id).mode(mode);

            Some(
                Context::osu_scores()
                    .top(legacy_scores)
                    .limit(100)
                    .exec(user_args)
                    .await,
            )
        } else {
            None
        }
    };

    with_miss_analyzer &= mode == GameMode::Osu;
    with_render &= mode == GameMode::Osu
        && orig.has_permission_to(Permissions::SEND_MESSAGES)
        && Context::ordr().is_some();

    #[cfg(feature = "twitch")]
    let twitch_fut = async {
        if let Some(user_id) = twitch_id {
            twitch_data(user_id).await
        } else {
            None
        }
    };

    #[cfg(feature = "twitch")]
    let (top100_res, twitch_data) = tokio::join!(top100_fut, twitch_fut);

    #[cfg(not(feature = "twitch"))]
    let top100_res = top100_fut.await;

    let top100 = match top100_res {
        Some(Ok(scores)) => Some(scores),
        None => None,
        Some(Err(err)) => {
            warn!(?err, "Failed to get top100");

            None
        }
    };

    let guild_id = orig.guild_id();
    let miss_analyzer = MissAnalyzerCheck::new(guild_id, with_miss_analyzer);
    let origin = MessageOrigin::new(guild_id, orig.channel_id());

    let mut entries = process_scores(
        scores,
        top100,
        #[cfg(feature = "twitch")]
        twitch_data,
        origin,
        legacy_scores,
        with_render,
        miss_analyzer,
    );

    let mut content = tries.map_or(SingleScoreContent::None, |tries| {
        SingleScoreContent::OnlyForIndex {
            idx: num,
            content: format!("Try #{tries}"),
        }
    });

    if missing_settings {
        let add_content_notices = {
            const MAX_NOTICES: usize = 4;
            const NOTICE_PERCENT: f64 = 0.25;

            let mut guard = Context::get().builder_notices.own(author);
            let notices = guard.entry().or_default();

            if *notices < MAX_NOTICES && thread_rng().gen_bool(NOTICE_PERCENT) {
                *notices += 1;

                Some(*notices)
            } else {
                None
            }
        };

        if let Some(notices) = add_content_notices {
            debug!(user = %author, notices, "Adding builder notice");

            let builder = InteractionCommands::get_command("builder").map_or_else(
                || "`/builder`".to_owned(),
                |cmd| cmd.mention("builder").to_string(),
            );

            let mut new_content =
                format!("✨ NEW: You can now use {builder} to customize your score format! ✨");

            match content {
                SingleScoreContent::OnlyForIndex {
                    ref mut content, ..
                } => {
                    new_content.push('\n');
                    new_content.push_str(content);
                    *content = new_content;
                }
                SingleScoreContent::None => content = SingleScoreContent::SameForAll(new_content),
                SingleScoreContent::SameForAll(_) => unreachable!(),
            }
        }
    }

    let graph = match entries.get_mut(num) {
        Some(entry) if matches!(settings.image, SettingsImage::ImageWithStrains) => {
            match entry.get_mut().await {
                Ok(entry) => {
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
                Err(err) => {
                    warn!(?err, "Failed to get score data");

                    None
                }
            }
        }
        Some(_) | None => None,
    };

    let mut pagination =
        SingleScorePagination::new(&user, entries, settings, score_data, author, content);

    pagination.set_index(num);

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .attachment(graph)
        .begin(orig)
        .await
}

fn process_scores(
    scores: Vec<Score>,
    top100: Option<Vec<Score>>,
    #[cfg(feature = "twitch")] twitch_data: Option<crate::commands::utility::TwitchData>,
    origin: MessageOrigin,
    legacy_scores: bool,
    with_render: bool,
    miss_analyzer: MissAnalyzerCheck,
) -> Box<[ScoreEmbedDataWrap]> {
    let top100 = top100.map(Arc::from);

    #[cfg(feature = "twitch")]
    let twitch_data = twitch_data.map(Arc::new);

    scores
        .into_iter()
        .map(|score| {
            ScoreEmbedDataWrap::new_raw(
                score,
                legacy_scores,
                with_render,
                miss_analyzer,
                top100.as_ref().map(Arc::clone),
                #[cfg(feature = "twitch")]
                twitch_data.as_ref().map(Arc::clone),
                origin,
            )
        })
        .collect()
}

#[cfg(feature = "twitch")]
async fn twitch_data(user_id: u64) -> Option<crate::commands::utility::TwitchData> {
    use crate::commands::utility::TwitchData;

    async fn get_vod(user_id: u64) -> Option<bathbot_model::TwitchVideo> {
        match Context::client().get_last_twitch_vod(user_id).await {
            Ok(Some(vod)) => Some(vod),
            Ok(None) => None,
            Err(err) => {
                warn!(?err, "Failed to get twitch vod");
                Context::online_twitch_streams().set_offline_by_user(user_id);

                None
            }
        }
    }

    async fn get_stream(user_id: u64) -> Option<bathbot_model::TwitchStream> {
        match Context::client().get_twitch_stream(user_id).await {
            Ok(Some(stream)) => Some(stream),
            Ok(None) => {
                // TODO: remove twitch id from user config

                None
            }
            Err(err) => {
                warn!(?err, "Failed to get twitch stream");
                Context::online_twitch_streams().set_offline_by_user(user_id);

                None
            }
        }
    }

    let stream = get_stream(user_id).await?;

    if !stream.live {
        return None;
    }

    {
        let online_twitch_streams = Context::online_twitch_streams();
        let guard = online_twitch_streams.guard();
        online_twitch_streams.set_online(&stream, &guard);
    }

    let data = match get_vod(user_id).await {
        Some(vod) => TwitchData::Vod {
            vod,
            stream_login: stream.login,
        },
        None => TwitchData::Stream {
            login: stream.login,
        },
    };

    Some(data)
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

async fn slash_rs(mut command: InteractionCommand) -> Result<()> {
    let args = Rs::from_interaction(command.input_data())?;

    score((&mut command).into(), args.into()).await
}

#[derive(Default)]
struct GuildValues {
    retries: Option<Retries>,
    render_button: Option<bool>,
    score_data: Option<ScoreData>,
}

impl From<&GuildConfig> for GuildValues {
    fn from(config: &GuildConfig) -> Self {
        Self {
            retries: config.retries,
            render_button: config.render_button,
            score_data: config.score_data,
        }
    }
}

async fn get_guild_values(orig: &CommandOrigin<'_>) -> GuildValues {
    match orig.guild_id() {
        Some(guild_id) => {
            Context::guild_config()
                .peek(guild_id, |config| GuildValues::from(config))
                .await
        }
        None => GuildValues::default(),
    }
}
