use super::ProfileSize;
use crate::{
    commands::{check_user_mention, parse_discord, parse_mode_option, DoubleResultCow},
    database::UserConfig,
    error::Error,
    util::{
        constants::common_literals::{DISCORD, MODE, NAME},
        ApplicationCommandExt, CowUtils, InteractionExt,
    },
    Args,  Context,
};

use twilight_model::{
    application::interaction::{application_command::CommandOptionValue, ApplicationCommand},
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
    ) -> DoubleResultCow<Self> {
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
                match check_user_mention(ctx, arg.as_ref()).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        Ok(Ok(Self { config }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;

        for option in command.yoink_options() {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    MODE => config.mode = parse_mode_option(&value),
                    "size" => match value.as_str() {
                        "compact" => config.profile_size = Some(ProfileSize::Compact),
                        "medium" => config.profile_size = Some(ProfileSize::Medium),
                        "full" => config.profile_size = Some(ProfileSize::Full),
                        _ => return Err(Error::InvalidCommandOptions),
                    },
                    NAME => config.osu = Some(value.into()),
                    _ => return Err(Error::InvalidCommandOptions),
                },
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

        Ok(Ok(Self { config }))
    }
}
