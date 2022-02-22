use std::{fmt::Write, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{GameMode, Grade, OsuError};
use twilight_model::{
    application::interaction::{
        application_command::{CommandDataOption, CommandOptionValue},
        ApplicationCommand,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        check_user_mention,
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        parse_discord, parse_mode_option, DoubleResultCow,
    },
    database::UserConfig,
    embeds::{EmbedData, RecentListEmbed},
    error::Error,
    pagination::{Pagination, RecentListPagination},
    util::{
        constants::{
            common_literals::{DISCORD, GRADE, MODE, MODS, NAME},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher, numbers,
        osu::ModSelection,
        InteractionExt, MessageBuilder, MessageExt,
    },
    Args, BotResult, CommandData, Context,
};

use super::GradeArg;

pub(super) async fn _recentlist(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RecentListArgs,
) -> BotResult<()> {
    let RecentListArgs {
        config,
        grade,
        mods,
    } = args;
    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    // Retrieve the user and their recent scores
    let user_args = UserArgs::new(name.as_str(), mode);

    let score_args = ScoreArgs::recent(100)
        .include_fails(grade.map_or(true, |g| g.include_fails()))
        .with_combo();

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((_, scores)) if scores.is_empty() => {
            let content = format!(
                "No recent {}plays found for user `{name}`",
                match mode {
                    GameMode::STD => "",
                    GameMode::TKO => "taiko ",
                    GameMode::CTB => "ctb ",
                    GameMode::MNA => "mania ",
                },
            );

            return data.error(&ctx, content).await;
        }
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

    match grade {
        Some(GradeArg::Single(grade)) => scores.retain(|score| score.grade == grade),
        Some(GradeArg::Range { bot, top }) => {
            scores.retain(|score| (bot..=top).contains(&score.grade))
        }
        None => {}
    }

    match mods {
        Some(ModSelection::Include(mods)) => scores.retain(|score| score.mods.contains(mods)),
        Some(ModSelection::Exact(mods)) => scores.retain(|score| score.mods == mods),
        Some(ModSelection::Exclude(mods)) => {
            scores.retain(|score| score.mods.intersection(mods).is_empty())
        }
        None => {}
    }

    let pages = numbers::div_euclid(10, scores.len());
    let scores_iter = scores.iter().take(10);

    let embed = match RecentListEmbed::new(&user, scores_iter, &ctx, (1, pages)).await {
        Ok(data) => data.into_builder().build(),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Creating the embed
    let mut builder = MessageBuilder::from(embed);

    if let Some(content) = message_content(grade, mods) {
        builder = builder.content(content);
    }

    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if scores.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = RecentListPagination::new(response, user, scores, Arc::clone(&ctx));
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

fn message_content(grade: Option<GradeArg>, mods: Option<ModSelection>) -> Option<String> {
    let mut content = String::new();

    match grade {
        Some(GradeArg::Single(grade)) => {
            let _ = write!(content, "`Grade: {grade}`");
        }
        Some(GradeArg::Range { bot, top }) => {
            let _ = write!(content, "`Grade: {bot} - {top}`");
        }
        None => {}
    }

    if let Some(selection) = mods {
        if grade.is_some() {
            content.push_str(" ~ ");
        }

        content.push_str("`Mods: ");

        match selection {
            ModSelection::Exact(_) => {}
            ModSelection::Exclude(_) => content.push_str("Exclude"),
            ModSelection::Include(_) => content.push_str("Include "),
        }

        let _ = write!(content, "{}`", selection.mods());
    }

    (!content.is_empty()).then(|| content)
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
    pub mods: Option<ModSelection>,
}

impl RecentListArgs {
    const ERR_PARSE_GRADE: &'static str = "Failed to parse `grade`.\n\
        Must be either a single grade or two grades of the form `a..b` e.g. `C..S`.\n\
        Valid grades are: `SSH`, `SS`, `SH`, `S`, `A`, `B`, `C`, `D`, or `F`";

    const ERR_PARSE_MODS: &'static str = "Failed to parse mods.\n\
        If you want included mods, specify it e.g. as `+hrdt`.\n\
        If you want exact mods, specify it e.g. as `+hdhr!`.\n\
        And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
        let mut grade = None;
        let mut passes = None;

        for arg in args.take(3) {
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

                            return Ok(Err(content.into()));
                        }
                    },
                    "fail" | "fails" | "f" => match value {
                        "true" | "t" | "1" => passes = Some(false),
                        "false" | "f" | "0" => passes = Some(true),
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
                            "Unrecognized option `{key}`.\n\
                            Available options are: `grade` or `pass`."
                        );

                        return Ok(Err(content.into()));
                    }
                }
            } else {
                match check_user_mention(ctx, arg).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
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

        Ok(Ok(Self {
            config,
            grade,
            mods: None,
        }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut grade = None;
        let mut mods = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    MODE => config.mode = parse_mode_option(&value),
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
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    MODS => match matcher::get_mods(&value) {
                        Some(mods_) => mods = Some(mods_),
                        None => return Ok(Err(Self::ERR_PARSE_MODS.into())),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Boolean(value) => {
                    let value = (option.name == "passes")
                        .then(|| value)
                        .ok_or(Error::InvalidCommandOptions)?;

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

        Ok(Ok(Self {
            config,
            grade,
            mods,
        }))
    }
}
