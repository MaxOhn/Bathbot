use crate::{
    embeds::{EmbedData, OsuStatsCountsEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    Args, BotResult, CommandData, Context, Name,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::application::interaction::application_command::CommandDataOption;

pub(super) async fn _count(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: CountArgs,
) -> BotResult<()> {
    let CountArgs { name, mode } = args;

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(data.author()?.id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    let user = match super::request_user(&ctx, &name, Some(mode)).await {
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

    let counts = match super::get_globals_count(&ctx, &user.username, mode).await {
        Ok(counts) => counts,
        Err(why) => {
            let content = "Some issue with the osustats website, blame bade";
            let _ = data.error(&ctx, content).await;

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
            match CountArgs::args(&ctx, &mut args, GameMode::STD) {
                Ok(count_args) => {
                    _count(ctx, CommandData::Message { msg, args, num }, count_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, command).await,
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
            match CountArgs::args(&ctx, &mut args, GameMode::MNA) {
                Ok(count_args) => {
                    _count(ctx, CommandData::Message { msg, args, num }, count_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, command).await,
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
            match CountArgs::args(&ctx, &mut args, GameMode::TKO) {
                Ok(count_args) => {
                    _count(ctx, CommandData::Message { msg, args, num }, count_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, command).await,
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
            match CountArgs::args(&ctx, &mut args, GameMode::CTB) {
                Ok(count_args) => {
                    _count(ctx, CommandData::Message { msg, args, num }, count_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_osustats(ctx, command).await,
    }
}

pub(super) struct CountArgs {
    name: Option<Name>,
    mode: GameMode,
}

impl CountArgs {
    fn args(ctx: &Context, args: &mut Args, mode: GameMode) -> Result<Self, &'static str> {
        let name = args
            .next()
            .map(|name| Args::try_link_name(ctx, name))
            .transpose()?;

        Ok(Self { name, mode })
    }

    pub(super) fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
    ) -> BotResult<Result<Self, String>> {
        let mut username = None;
        let mut mode = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "name" => username = Some(value.into()),
                    "discord" => username = parse_discord_option!(ctx, value, "osustats count"),
                    "mode" => mode = parse_mode_option!(value, "osustats count"),
                    _ => bail_cmd_option!("osustats count", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("osustats count", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("osustats count", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("osustats count", subcommand, name)
                }
            }
        }

        let args = Self {
            name: username,
            mode: mode.unwrap_or(GameMode::STD),
        };

        Ok(Ok(args))
    }
}
