use super::ProfileSize;
use crate::{
    util::{ApplicationCommandExt, CowUtils},
    Args, BotResult, Context, Name,
};

use rosu_v2::prelude::GameMode;
use std::borrow::Cow;
use twilight_model::application::interaction::{
    application_command::CommandDataOption, ApplicationCommand,
};

pub(super) struct ProfileArgs {
    pub name: Option<Name>,
    pub mode: GameMode,
    pub kind: Option<ProfileSize>,
}

impl ProfileArgs {
    pub(super) fn args(
        ctx: &Context,
        args: &mut Args,
        mode: GameMode,
    ) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut kind = None;

        for arg in args.take(2).map(CowUtils::cow_to_ascii_lowercase) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = &arg[idx + 1..];

                match key {
                    "size" => {
                        kind = match value {
                            "compact" | "small" => Some(ProfileSize::Compact),
                            "medium" => Some(ProfileSize::Medium),
                            "full" | "big" => Some(ProfileSize::Full),
                            _ => {
                                let content = "Failed to parse `size`. Must be either `compact`, `medium`, or `full`.";

                                return Err(content.into());
                            }
                        };
                    }
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `size`.",
                            key
                        );

                        return Err(content.into());
                    }
                }
            } else {
                name = Some(Args::try_link_name(ctx, arg.as_ref())?);
            }
        }

        Ok(Self { name, mode, kind })
    }

    pub(super) fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, String>> {
        let mut username = None;
        let mut mode = None;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "mode" => mode = parse_mode_option!(value, "profile"),
                    "size" => match value.as_str() {
                        "compact" => kind = Some(ProfileSize::Compact),
                        "medium" => kind = Some(ProfileSize::Medium),
                        "full" => kind = Some(ProfileSize::Full),
                        _ => bail_cmd_option!("profile size", string, value),
                    },
                    "name" => username = Some(value.into()),
                    "discord" => username = parse_discord_option!(ctx, value, "profile"),
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

        let args = Self {
            name: username,
            mode: mode.unwrap_or(GameMode::STD),
            kind,
        };

        Ok(Ok(args))
    }
}
