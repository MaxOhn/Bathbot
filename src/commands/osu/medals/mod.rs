mod common;
mod medal;
mod missing;
mod recent;
mod stats;

pub use common::*;
pub use medal::*;
pub use missing::*;
pub use recent::*;
pub use stats::*;

use super::{request_user, require_link};
use crate::{
    commands::SlashCommandBuilder,
    util::{ApplicationCommandExt, MessageExt},
    BotResult, Context, Error, Name,
};

use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption,
        OptionsCommandOptionData,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

enum MedalCommandKind {
    Common(CommonArgs),
    Medal(String),
    Missing(Option<Name>),
    Recent(RecentArgs),
    Stats(Option<Name>),
}

impl MedalCommandKind {
    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let author_id = command.user_id()?;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => bail_cmd_option!("medal", string, name),
                CommandDataOption::Integer { name, .. } => bail_cmd_option!("medal", integer, name),
                CommandDataOption::Boolean { name, .. } => bail_cmd_option!("medal", boolean, name),
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "common" => match CommonArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(Self::Common(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "info" => {
                        let mut medal_name = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "name" => medal_name = Some(value),
                                    _ => bail_cmd_option!("medal info", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("medal info", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("medal info", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("medal info", subcommand, name)
                                }
                            }
                        }

                        let name = medal_name.ok_or(Error::InvalidCommandOptions)?;
                        kind = Some(MedalCommandKind::Medal(name));
                    }
                    "stats" => {
                        let mut username = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "name" => username = Some(value.into()),
                                    "discord" => {
                                        username = parse_discord_option!(ctx, value, "medal stats")
                                    }
                                    _ => bail_cmd_option!("medal stats", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("medal stats", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("medal stats", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("medal stats", subcommand, name)
                                }
                            }
                        }

                        let name = match username {
                            Some(name) => Some(name),
                            None => ctx.user_config(author_id).await?.osu_username,
                        };

                        kind = Some(MedalCommandKind::Stats(name));
                    }
                    "missing" => {
                        let mut username = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "name" => username = Some(value.into()),
                                    "discord" => {
                                        username =
                                            parse_discord_option!(ctx, value, "medal missing")
                                    }
                                    _ => bail_cmd_option!("medal missing", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("medal missing", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("medal missing", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("medal missing", subcommand, name)
                                }
                            }
                        }

                        let name = match username {
                            Some(name) => Some(name),
                            None => ctx.user_config(author_id).await?.osu_username,
                        };

                        kind = Some(MedalCommandKind::Missing(name));
                    }
                    "recent" => match RecentArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(Self::Recent(args)),
                        Err(content) => return Ok(Err(content.into())),
                    },
                    _ => bail_cmd_option!("medal", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }
}

pub async fn slash_medal(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match MedalCommandKind::slash(&ctx, &mut command).await? {
        Ok(MedalCommandKind::Common(args)) => _common(ctx, command.into(), args).await,
        Ok(MedalCommandKind::Medal(name)) => _medal(ctx, command.into(), &name).await,
        Ok(MedalCommandKind::Missing(config)) => _medalsmissing(ctx, command.into(), config).await,
        Ok(MedalCommandKind::Recent(args)) => _medalrecent(ctx, command.into(), args).await,
        Ok(MedalCommandKind::Stats(config)) => _medalstats(ctx, command.into(), config).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn slash_medal_command() -> Command {
    let description = "Info about a medal or users' medal progress";

    let options = vec![
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "Compare which of the given users achieved medals first".to_owned(),
            name: "common".to_owned(),
            options: vec![
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Specify a username".to_owned(),
                    name: "name1".to_owned(),
                    required: false,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Specify a username".to_owned(),
                    name: "name2".to_owned(),
                    required: false,
                }),
                CommandOption::User(BaseCommandOptionData {
                    description: "Specify a linked discord user".to_owned(),
                    name: "discord1".to_owned(),
                    required: false,
                }),
                CommandOption::User(BaseCommandOptionData {
                    description: "Specify a linked discord user".to_owned(),
                    name: "discord2".to_owned(),
                    required: false,
                }),
            ],
            required: false,
        }),
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "Display info about an osu! medal".to_owned(),
            name: "info".to_owned(),
            options: vec![CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify the name of the medal".to_owned(),
                name: "name".to_owned(),
                required: true,
            })],
            required: false,
        }),
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "Display info about an osu! medal".to_owned(),
            name: "missing".to_owned(),
            options: vec![
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Specify a username".to_owned(),
                    name: "name".to_owned(),
                    required: false,
                }),
                CommandOption::User(BaseCommandOptionData {
                    description: "Specify a linked discord user".to_owned(),
                    name: "discord".to_owned(),
                    required: false,
                }),
            ],
            required: false,
        }),
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "Display a recently acquired medal of a user".to_owned(),
            name: "recent".to_owned(),
            options: vec![
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Specify a username".to_owned(),
                    name: "name".to_owned(),
                    required: false,
                }),
                CommandOption::Integer(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Specify an index e.g. 1 = most recent".to_owned(),
                    name: "index".to_owned(),
                    required: false,
                }),
                CommandOption::User(BaseCommandOptionData {
                    description: "Specify a linked discord user".to_owned(),
                    name: "discord".to_owned(),
                    required: false,
                }),
            ],
            required: false,
        }),
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "Display medal stats for a user".to_owned(),
            name: "stats".to_owned(),
            options: vec![
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Specify a username".to_owned(),
                    name: "name".to_owned(),
                    required: false,
                }),
                CommandOption::User(BaseCommandOptionData {
                    description: "Specify a linked discord user".to_owned(),
                    name: "discord".to_owned(),
                    required: false,
                }),
            ],
            required: false,
        }),
    ];

    SlashCommandBuilder::new("medal", description)
        .options(options)
        .build()
}
