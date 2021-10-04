use super::ProfileSize;
use crate::{
    database::UserConfig,
    util::{
        constants::common_literals::{DISCORD, MODE, NAME},
        ApplicationCommandExt, CowUtils, InteractionExt,
    },
    Args, BotResult, Context,
};

use rosu_v2::prelude::GameMode;
use std::borrow::Cow;
use twilight_model::{
    application::interaction::{application_command::CommandDataOption, ApplicationCommand},
    id::UserId,
};

pub(super) struct ProfileArgs {
    pub config: UserConfig,
}

impl ProfileArgs {
    pub(super) async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut config = ctx.user_config(author_id).await?;

        for arg in args.take(2).map(CowUtils::cow_to_ascii_lowercase) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = &arg[idx + 1..];

                match key {
                    "size" => {
                        config.profile_size = match value {
                            "compact" | "small" => Some(ProfileSize::Compact),
                            "medium" => Some(ProfileSize::Medium),
                            "full" | "big" => Some(ProfileSize::Full),
                            _ => {
                                let content = "Failed to parse `size`. Must be either `compact`, `medium`, or `full`.";

                                return Ok(Err(content.into()));
                            }
                        };
                    }
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `size`.",
                            key
                        );

                        return Ok(Err(content.into()));
                    }
                }
            } else {
                match Args::check_user_mention(ctx, arg.as_ref()).await? {
                    Ok(name) => config.osu_username = Some(name),
                    Err(content) => return Ok(Err(content.into())),
                }
            }
        }

        Ok(Ok(Self { config }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, String>> {
        let mut config = ctx.user_config(command.user_id()?).await?;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    MODE => config.mode = parse_mode_option!(value, "profile"),
                    "size" => match value.as_str() {
                        "compact" => config.profile_size = Some(ProfileSize::Compact),
                        "medium" => config.profile_size = Some(ProfileSize::Medium),
                        "full" => config.profile_size = Some(ProfileSize::Full),
                        _ => bail_cmd_option!("profile size", string, value),
                    },
                    NAME => config.osu_username = Some(value.into()),
                    DISCORD => config.osu_username = parse_discord_option!(ctx, value, "profile"),
                    _ => bail_cmd_option!("profile", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("profile", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("profile", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("profile", subcommand, name)
                }
            }
        }

        Ok(Ok(Self { config }))
    }
}
