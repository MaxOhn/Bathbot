use super::{ErrorType, GradeArg};
use crate::{
    database::UserConfig,
    embeds::{EmbedData, RecentListEmbed},
    pagination::{Pagination, RecentListPagination},
    util::{
        constants::{
            common_literals::{DISCORD, GRADE, MODE, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        numbers, MessageExt,
    },
    Args, BotResult, CommandData, Context,
};

use eyre::Report;
use futures::future::TryFutureExt;
use rosu_v2::prelude::{GameMode, Grade, OsuError};
use std::{borrow::Cow, sync::Arc};
use twilight_model::{
    application::interaction::application_command::CommandDataOption, id::UserId,
};

pub(super) async fn _recentlist(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RecentListArgs,
) -> BotResult<()> {
    let RecentListArgs { config, grade } = args;
    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    // Retrieve the user and their recent scores
    let user_fut = super::request_user(&ctx, &name, mode).map_err(From::from);

    let scores_fut = ctx
        .osu()
        .user_scores(name.as_str())
        .recent()
        .mode(mode)
        .limit(100)
        .include_fails(grade.map_or(true, |g| g.include_fails()));

    let scores_fut = super::prepare_scores(&ctx, scores_fut);

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
                name
            );

            return data.error(&ctx, content).await;
        }
        Ok((user, scores)) => (user, scores),
        Err(ErrorType::Osu(OsuError::NotFound)) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(ErrorType::Osu(why)) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
        Err(ErrorType::Bot(why)) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
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

    let pages = numbers::div_euclid(10, scores.len());
    let scores_iter = scores.iter().take(10);

    let embed = match RecentListEmbed::new(&user, scores_iter, (1, pages)).await {
        Ok(data) => data.into_builder().build(),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let builder = embed.into();
    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = RecentListPagination::new(Arc::clone(&ctx), response, user, scores);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display a list of a user's most recent plays")]
#[long_desc(
    "Display a list of a user's most recent plays.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rl")]
pub async fn recentlist(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentListArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode.get_or_insert(GameMode::STD);

                    _recentlist(ctx, CommandData::Message { msg, args, num }, recent_args).await
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
#[short_desc("Display a list of a user's most recent mania plays")]
#[long_desc(
    "Display a list of a user's most recent mania plays.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rlm")]
pub async fn recentlistmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentListArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::MNA);

                    _recentlist(ctx, CommandData::Message { msg, args, num }, recent_args).await
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
#[short_desc("Display a list of a user's most recent taiko plays")]
#[long_desc(
    "Display a list of a user's most recent taiko plays.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rlt")]
pub async fn recentlisttaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentListArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::TKO);

                    _recentlist(ctx, CommandData::Message { msg, args, num }, recent_args).await
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
#[short_desc("Display a list of a user's most recent ctb plays")]
#[long_desc(
    "Display a list of a user's most recent ctb plays.\n\
    To filter all fails, you can specify `pass=true`.\n\
    To filter specific grades, you can specify `grade=...` where you can provide \
    either a single grade or a grade *range*.\n\
    Ranges can be specified like\n\
    - `a..b` e.g. `C..SH` to only keep scores with grades between C and SH\n\
    - `a..` e.g. `C..` to only keep scores with grade C or higher\n\
    - `..b` e.g. `..C` to only keep scores that have at most grade C\n\
    Available grades are `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rlc")]
pub async fn recentlistctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RecentListArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut recent_args)) => {
                    recent_args.config.mode = Some(GameMode::CTB);

                    _recentlist(ctx, CommandData::Message { msg, args, num }, recent_args).await
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

pub(super) struct RecentListArgs {
    pub config: UserConfig,
    pub grade: Option<GradeArg>,
}

impl RecentListArgs {
    const ERR_PARSE_GRADE: &'static str = "Failed to parse `grade`.\n\
        Must be either a single grade or two grades of the form `a..b` e.g. `C..S`.\n\
        Valid grades are: `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`";

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(author_id).await?;
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
                    Ok(osu) => config.osu = Some(osu),
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

        Ok(Ok(Self { config, grade }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(author_id).await?;
        let mut grade = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    DISCORD => config.osu = Some(parse_discord_option!(ctx, value, "recent list")),
                    MODE => config.mode = parse_mode_option!(value, "recent list"),
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
                        _ => bail_cmd_option!("recent list grade", string, value),
                    },
                    _ => bail_cmd_option!("recent list", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("recent list", integer, name)
                }
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
                    _ => bail_cmd_option!("recent list", boolean, name),
                },
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("recent list", subcommand, name)
                }
            }
        }

        Ok(Ok(Self { config, grade }))
    }
}
