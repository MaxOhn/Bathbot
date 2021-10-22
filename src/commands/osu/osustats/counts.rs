use crate::{
    database::UserConfig,
    embeds::{EmbedData, OsuStatsCountsEmbed},
    util::{
        constants::{
            common_literals::{DISCORD, MODE, NAME},
            GENERAL_ISSUE, OSUSTATS_API_ISSUE, OSU_API_ISSUE,
        },
        MessageExt,
    },
    Args, BotResult, CommandData, Context,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::{
    application::interaction::application_command::CommandDataOption, id::UserId,
};

pub(super) async fn _count(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: CountArgs,
) -> BotResult<()> {
    let CountArgs { config } = args;
    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let mut user = match super::request_user(&ctx, &name, mode).await {
        Ok(user) => user,
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

    let counts = match super::get_globals_count(&ctx, &user, mode).await {
        Ok(counts) => counts,
        Err(why) => {
            let _ = data.error(&ctx, OSUSTATS_API_ISSUE).await;

            return Err(why);
        }
    };

    let embed_data = OsuStatsCountsEmbed::new(user, mode, counts);
    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}

#[command]
#[short_desc("Count how often a user appears on top of a map's leaderboard")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `osu` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("osc", "osustatscounts")]
pub async fn osustatscount(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match CountArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut count_args)) => {
                    count_args.config.mode.get_or_insert(GameMode::STD);

                    _count(ctx, CommandData::Message { msg, args, num }, count_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, *command).await,
    }
}

#[command]
#[short_desc("Count how often a user appears on top of a mania map's leaderboard")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `mania` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("oscm", "osustatscountsmania")]
pub async fn osustatscountmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match CountArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut count_args)) => {
                    count_args.config.mode = Some(GameMode::MNA);

                    _count(ctx, CommandData::Message { msg, args, num }, count_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, *command).await,
    }
}

#[command]
#[short_desc("Count how often a user appears on top of a taiko map's leaderboard")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `taiko` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("osct", "osustatscountstaiko")]
pub async fn osustatscounttaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match CountArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut count_args)) => {
                    count_args.config.mode = Some(GameMode::TKO);

                    _count(ctx, CommandData::Message { msg, args, num }, count_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, *command).await,
    }
}

#[command]
#[short_desc("Count how often a user appears on top of a ctb map's leaderboard")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `ctb` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("oscc", "osustatscountsctb")]
pub async fn osustatscountctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match CountArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut count_args)) => {
                    count_args.config.mode = Some(GameMode::CTB);

                    _count(ctx, CommandData::Message { msg, args, num }, count_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, *command).await,
    }
}

pub(super) struct CountArgs {
    config: UserConfig,
}

const OSUSTATS_COUNT: &str = "osustats count";

impl CountArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
    ) -> BotResult<Result<Self, &'static str>> {
        let mut config = ctx.user_config(author_id).await?;

        if let Some(arg) = args.next() {
            match Args::check_user_mention(ctx, arg).await? {
                Ok(osu) => config.osu = Some(osu),
                Err(content) => return Ok(Err(content)),
            }
        }

        Ok(Ok(Self { config }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, String>> {
        let mut config = ctx.user_config(author_id).await?;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    DISCORD => {
                        config.osu = Some(parse_discord_option!(ctx, value, "osustats count"))
                    }
                    MODE => config.mode = parse_mode_option!(value, "osustats count"),
                    _ => bail_cmd_option!(OSUSTATS_COUNT, string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!(OSUSTATS_COUNT, integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(OSUSTATS_COUNT, boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!(OSUSTATS_COUNT, subcommand, name)
                }
            }
        }

        Ok(Ok(Self { config }))
    }
}
