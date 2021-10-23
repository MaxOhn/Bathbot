use super::GradeArg;
use crate::{
    database::UserConfig,
    embeds::{EmbedData, RecentEmbed},
    tracking::process_tracking,
    twitch::TwitchVideo,
    util::{
        constants::{
            common_literals::{DISCORD, GRADE, INDEX, MODE, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        CowUtils, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder, Name,
};

use rosu_v2::prelude::{
    Beatmap, GameMode, GameMods, Grade, OsuError,
    RankStatus::{Approved, Loved, Qualified, Ranked},
    Score,
};
use std::{borrow::Cow, fmt::Write, mem, sync::Arc};
use tokio::time::{sleep, Duration};
use twilight_model::{
    application::interaction::application_command::CommandDataOption, id::UserId,
};

pub(super) async fn _recent(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RecentArgs,
) -> BotResult<()> {
    let RecentArgs {
        config,
        input_name,
        index,
        grade,
    } = args;

    let mut twitch_id = None;

    let name = match (config.username(), &input_name) {
        (Some(name), None) => {
            twitch_id = Some(config.twitch_id);

            name.as_str()
        }
        (Some(name_config), Some(name_input)) => {
            let name_config_lower = name_config.cow_to_ascii_lowercase();
            let name_input_lower = name_input.cow_to_ascii_lowercase();

            if name_config_lower == name_input_lower {
                twitch_id = Some(config.twitch_id);

                name_config.as_str()
            } else {
                name_input.as_str()
            }
        }
        (None, Some(name)) => name.as_str(),
        (None, None) => return super::require_link(&ctx, &data).await,
    };

    let mode = config.mode.unwrap_or(GameMode::STD);

    // Retrieve the user and their recent scores
    let user_fut = super::request_user(&ctx, name, mode);

    let scores_fut = ctx
        .osu()
        .user_scores(name)
        .recent()
        .mode(mode)
        .limit(100)
        .include_fails(grade.map_or(true, |g| g.include_fails()));

    let (mut user, mut scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((_, scores)) if scores.is_empty() => {
            let content = format!(
                "No recent {}plays found for user `{}`",
                match mode {
                    GameMode::STD => "",
                    GameMode::TKO => "taiko ",
                    GameMode::CTB => "ctb ",
                    GameMode::MNA => "mania ",
                },
                name,
            );

            return data.error(&ctx, content).await;
        }
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    match grade {
        Some(GradeArg::Single(grade)) => scores.retain(|score| score.grade == grade),
        Some(GradeArg::Range { bot, top }) => {
            scores.retain(|score| (bot..=top).contains(&score.grade))
        }
        None => {}
    }

    let num = index.unwrap_or(1).saturating_sub(1);
    let mut iter = scores.iter_mut().skip(num);

    let (score, tries) = match iter.next() {
        Some(score) => match super::prepare_score(&ctx, score).await {
            Ok(_) => {
                let mods = score.mods;
                let map_id = map_id!(score).unwrap();

                let tries = 1 + iter
                    .take_while(|s| map_id!(s).unwrap() == map_id && s.mods == mods)
                    .count();

                (score, tries)
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
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

            return data.error(&ctx, content).await;
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
                Err(why) => {
                    unwind_error!(warn, why, "Failed to get config of input name: {}");

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
        Some(Err(why)) => {
            unwind_error!(warn, why, "Error while getting global scores: {}");

            None
        }
    };

    let mut best = match best_result {
        None => None,
        Some(Ok(scores)) => Some(scores),
        Some(Err(why)) => {
            unwind_error!(warn, why, "Error while getting top scores: {}");

            None
        }
    };

    let data_fut = RecentEmbed::new(
        &user,
        score,
        best.as_deref(),
        map_score.as_ref(),
        twitch_vod,
    );

    let embed_data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let content = config.show_retries().then(|| format!("Try #{}", tries));

    // Only maximize if config allows it
    if config.embeds_maximized() {
        let embed = embed_data.as_builder().build();
        let mut builder = MessageBuilder::new().embed(embed);

        if let Some(content) = content {
            builder = builder.content(content);
        }

        let mut response = data.create_message(&ctx, builder).await?.model().await?;
        ctx.store_msg(response.id);

        // Set map on garbage collection list if unranked
        let gb = ctx.map_garbage_collector(map);

        // * Note: Don't store maps in DB as their max combo isn't available

        // Process user and their top scores for tracking
        if let Some(ref mut scores) = best {
            if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
                unwind_error!(warn, why, "Error while storing best maps in DB: {}");
            }

            process_tracking(&ctx, mode, scores, Some(&user)).await;
        }

        // Wait for minimizing
        tokio::spawn(async move {
            gb.execute(&ctx).await;
            sleep(Duration::from_secs(45)).await;

            if !ctx.remove_msg(response.id) {
                return;
            }

            let embed = embed_data.into_builder().build();
            let mut builder = MessageBuilder::new().embed(embed);

            if !response.content.is_empty() {
                builder = builder.content(mem::take(&mut response.content));
            }

            if let Err(why) = response.update_message(&ctx, builder).await {
                unwind_error!(warn, why, "Error minimizing recent msg: {}");
            }
        });
    } else {
        let embed = embed_data.into_builder().build();
        let mut builder = MessageBuilder::new().embed(embed);

        if let Some(content) = content {
            builder = builder.content(content);
        }

        data.create_message(&ctx, builder).await?;

        // Set map on garbage collection list if unranked
        let gb = ctx.map_garbage_collector(map);
        gb.execute(&ctx).await;

        // * Note: Don't store maps in DB as their max combo isn't available

        // Process user and their top scores for tracking
        if let Some(ref mut scores) = best {
            if let Err(why) = ctx.psql().store_scores_maps(scores.iter()).await {
                unwind_error!(warn, why, "Error while storing best maps in DB: {}");
            }

            process_tracking(&ctx, mode, scores, Some(&user)).await;
        }
    }

    Ok(())
}

async fn retrieve_vod(
    ctx: &Context,
    user_id: u64,
    score: &Score,
    map: &Beatmap,
) -> Option<TwitchVideo> {
    match ctx.clients.twitch.get_last_vod(user_id).await {
        Ok(Some(mut vod)) => {
            let vod_start = vod.created_at.timestamp();
            let vod_end = vod_start + vod.duration as i64;

            // Adjust map length with mods
            let mut map_length = if score.mods.contains(GameMods::DoubleTime) {
                map.seconds_drain as f32 * 2.0 / 3.0
            } else if score.mods.contains(GameMods::HalfTime) {
                map.seconds_drain as f32 * 4.0 / 3.0
            } else {
                map.seconds_drain as f32
            };

            // Adjust map length with passed objects in case of fail
            if score.grade == Grade::F {
                let passed = score.total_hits() as f32;
                let total = map.count_objects() as f32;

                map_length *= passed / total;
            }

            // 3 seconds early just to be sure
            let map_start = score.created_at.timestamp() - map_length as i64 - 3;

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
                let _ = write!(vod.url, "{}s", offset);
            }

            Some(vod)
        }
        Ok(None) => None,
        Err(why) => {
            unwind_error!(warn, why, "Failed to get twitch vod: {}");

            None
        }
    }
}

#[command]
#[short_desc("Display a user's most recent play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `r42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[example("badewanne3 pass=true", "grade=a", "whitecat grade=B..sh")]
#[aliases("r", "rs")]
pub async fn recent(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode.get_or_insert(GameMode::STD);

                    _recent(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display a user's most recent mania play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rm42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[example("badewanne3 pass=true", "grade=a", "whitecat grade=B..sh")]
#[aliases("rm")]
pub async fn recentmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::MNA);

                    _recent(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display a user's most recent taiko play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rt42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[example("badewanne3 pass=true", "grade=a", "whitecat grade=B..sh")]
#[aliases("rt")]
pub async fn recenttaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::TKO);

                    _recent(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

#[command]
#[short_desc("Display a user's most recent ctb play")]
#[long_desc(
    "Display a user's most recent play.\n\
    To get a previous recent score, you can add a number right after the command,\n\
    e.g. `rc42 badewanne3` to get the 42nd most recent score.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`.\n\n\
    With the `config` command you can set the embed as minimized immediately, \
    hide the retry count, and show your twitch stream and live VOD."
)]
#[usage("[username] [pass=true/false] [grade=grade[..grade]]")]
#[example("badewanne3 pass=true", "grade=a", "whitecat grade=B..sh")]
#[aliases("rc")]
pub async fn recentctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentArgs::args(&ctx, &mut args, msg.author.id, num).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::CTB);

                    _recent(ctx, CommandData::Message { msg, args, num }, recent_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_recent(ctx, *command).await,
    }
}

pub(super) struct RecentArgs {
    config: UserConfig,
    input_name: Option<Name>,
    index: Option<usize>,
    grade: Option<GradeArg>,
}

impl RecentArgs {
    const ERR_PARSE_GRADE: &'static str = "Failed to parse `grade`.\n\
        Must be either a single grade or two grades of the form `a..b` e.g. `C..S`.\n\
        Valid grades are: `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`";

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
        index: Option<usize>,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let config = ctx.user_config(author_id).await?;
        let mut input_name = None;
        let mut grade = None;
        let mut passes = None;

        for arg in args.take(3) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "pass" | "p" | "passes" => match value {
                        "true" | "1" => passes = Some(true),
                        "false" | "0" => passes = Some(false),
                        _ => {
                            let content =
                                "Failed to parse `pass`. Must be either `true` or `false`.";

                            return Ok(Err(content.into()));
                        }
                    },
                    "fail" | "fails" | "f" => match value {
                        "true" | "1" => passes = Some(false),
                        "false" | "0" => passes = Some(true),
                        _ => {
                            let content =
                                "Failed to parse `fail`. Must be either `true` or `false`.";

                            return Ok(Err(content.into()));
                        }
                    },
                    GRADE | "g" => match value.find("..") {
                        Some(idx) => {
                            let bot = &value[..idx];
                            let top = &value[idx + 2..];

                            let min = if bot.is_empty() {
                                Grade::XH
                            } else if let Ok(grade) = bot.parse() {
                                grade
                            } else {
                                return Ok(Err(Self::ERR_PARSE_GRADE.into()));
                            };

                            let max = if top.is_empty() {
                                Grade::D
                            } else if let Ok(grade) = top.parse() {
                                grade
                            } else {
                                return Ok(Err(Self::ERR_PARSE_GRADE.into()));
                            };

                            let bot = if min < max { min } else { max };
                            let top = if min > max { min } else { max };

                            grade = Some(GradeArg::Range { bot, top })
                        }
                        None => match value.parse().map(GradeArg::Single) {
                            Ok(grade_) => grade = Some(grade_),
                            Err(_) => return Ok(Err(Self::ERR_PARSE_GRADE.into())),
                        },
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `grade` or `pass`.",
                            key
                        );

                        return Ok(Err(content.into()));
                    }
                }
            } else {
                match Args::check_user_mention(ctx, arg).await? {
                    Ok(osu) => input_name = Some(osu.into_username()),
                    Err(content) => return Ok(Err(content.into())),
                }
            }
        }

        grade = match passes {
            Some(true) => match grade {
                Some(GradeArg::Single(Grade::F)) => None,
                Some(GradeArg::Single(_)) => grade,
                Some(GradeArg::Range { bot, top }) => match (bot, top) {
                    (Grade::F, Grade::F) => None,
                    (Grade::F, _) => Some(GradeArg::Range { bot: Grade::D, top }),
                    (_, Grade::F) => Some(GradeArg::Range {
                        bot: Grade::D,
                        top: bot,
                    }),
                    _ => Some(GradeArg::Range { bot, top }),
                },
                None => Some(GradeArg::Range {
                    bot: Grade::D,
                    top: Grade::XH,
                }),
            },
            Some(false) => Some(GradeArg::Single(Grade::F)),
            None => grade,
        };

        let args = Self {
            config,
            input_name,
            index,
            grade,
        };

        Ok(Ok(args))
    }

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(author_id).await?;
        let mut input_name = None;
        let mut index = None;
        let mut grade = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    NAME => input_name = Some(value.into()),
                    DISCORD => {
                        input_name =
                            Some(parse_discord_option!(ctx, value, "recent score").into_username())
                    }
                    MODE => config.mode = parse_mode_option!(value, "recent score"),
                    GRADE => match value.as_str() {
                        "SS" => {
                            grade = Some(GradeArg::Range {
                                bot: Grade::X,
                                top: Grade::XH,
                            })
                        }
                        "S" => {
                            grade = Some(GradeArg::Range {
                                bot: Grade::S,
                                top: Grade::SH,
                            })
                        }
                        "A" => grade = Some(GradeArg::Single(Grade::A)),
                        "B" => grade = Some(GradeArg::Single(Grade::B)),
                        "C" => grade = Some(GradeArg::Single(Grade::C)),
                        "D" => grade = Some(GradeArg::Single(Grade::D)),
                        "F" => grade = Some(GradeArg::Single(Grade::F)),
                        _ => bail_cmd_option!("recent score grade", string, value),
                    },
                    _ => bail_cmd_option!("recent score", string, name),
                },
                CommandDataOption::Integer { name, value } => match name.as_str() {
                    INDEX => index = Some(value.max(1).min(50) as usize),
                    _ => bail_cmd_option!("recent score", integer, name),
                },
                CommandDataOption::Boolean { name, value } => match name.as_str() {
                    "passes" => {
                        if value {
                            grade = match grade {
                                Some(GradeArg::Single(Grade::F)) => None,
                                Some(GradeArg::Single(_)) => grade,
                                Some(GradeArg::Range { .. }) => grade,
                                None => Some(GradeArg::Range {
                                    bot: Grade::D,
                                    top: Grade::XH,
                                }),
                            }
                        } else {
                            grade = Some(GradeArg::Single(Grade::F));
                        }
                    }
                    _ => bail_cmd_option!("recent score", boolean, name),
                },
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("recent score", subcommand, name)
                }
            }
        }

        let args = Self {
            config,
            input_name,
            index,
            grade,
        };

        Ok(Ok(args))
    }
}
