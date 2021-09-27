mod pp;
mod rank_pp;
mod rank_score;

pub use pp::*;
pub use rank_pp::*;
pub use rank_score::*;

use super::{request_user, require_link};

use crate::{
    commands::SlashCommandBuilder,
    util::{ApplicationCommandExt, MessageExt},
    BotResult, Context, Error,
};

use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption,
        OptionsCommandOptionData,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

enum ReachCommandKind {
    Performance(PpArgs),
    RankPerformance(RankPpArgs),
    RankScore(RankScoreArgs),
}

impl ReachCommandKind {
    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let author_id = command.user_id()?;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => bail_cmd_option!("reach", string, name),
                CommandDataOption::Integer { name, .. } => bail_cmd_option!("reach", integer, name),
                CommandDataOption::Boolean { name, .. } => bail_cmd_option!("reach", boolean, name),
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "pp" => match PpArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(Self::Performance(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "rank" => {
                        for option in options {
                            match option {
                                CommandDataOption::String { name, .. } => {
                                    bail_cmd_option!("reach rank", string, name)
                                }
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("reach rank", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("reach rank", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, options } => {
                                    match name.as_str() {
                                        "pp" => match RankPpArgs::slash(ctx, options, author_id)
                                            .await?
                                        {
                                            Ok(args) => kind = Some(Self::RankPerformance(args)),
                                            Err(content) => return Ok(Err(content.into())),
                                        },
                                        "score" => {
                                            match RankScoreArgs::slash(ctx, options, author_id)
                                                .await?
                                            {
                                                Ok(args) => kind = Some(Self::RankScore(args)),
                                                Err(content) => return Ok(Err(content.into())),
                                            }
                                        }
                                        _ => bail_cmd_option!("reach rank", subcommand, name),
                                    }
                                }
                            }
                        }
                    }
                    _ => bail_cmd_option!("rank", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }
}

pub async fn slash_reach(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match ReachCommandKind::slash(&ctx, &mut command).await? {
        Ok(ReachCommandKind::Performance(args)) => _pp(ctx, command.into(), args).await,
        Ok(ReachCommandKind::RankPerformance(args)) => _rank(ctx, command.into(), args).await,
        Ok(ReachCommandKind::RankScore(args)) => _rankscore(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn slash_reach_command() -> Command {
    let description = "How much is a user missing to reach the given pp or rank?";

    let options = vec![
        CommandOption::SubCommand(OptionsCommandOptionData {
            description: "How many pp is a user missing to reach the given amount?".to_owned(),
            name: "pp".to_owned(),
            options: vec![
                // TODO: Number
                CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Specify a target pp amount".to_owned(),
                    name: "pp".to_owned(),
                    required: true,
                }),
                CommandOption::String(ChoiceCommandOptionData {
                    choices: super::mode_choices(),
                    description: "Specify the gamemode".to_owned(),
                    name: "mode".to_owned(),
                    required: false,
                }),
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
        CommandOption::SubCommandGroup(OptionsCommandOptionData {
            description: "How many pp are missing to reach the given rank?".to_owned(),
            name: "rank".to_owned(),
            options: vec![
                CommandOption::SubCommand(OptionsCommandOptionData {
                    description: "How many pp is a user missing to reach the given rank?"
                        .to_owned(),
                    name: "pp".to_owned(),
                    options: vec![
                        CommandOption::Integer(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Specify the target rank".to_owned(),
                            name: "rank".to_owned(),
                            required: true,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: super::mode_choices(),
                            description: "Specify a gamemode".to_owned(),
                            name: "mode".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Specify a username".to_owned(),
                            name: "name".to_owned(),
                            required: false,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Specify a country (code)".to_owned(),
                            name: "country".to_owned(),
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
                    description: "How much ranked score is a user missing to reach the given rank?"
                        .to_owned(),
                    name: "score".to_owned(),
                    options: vec![
                        CommandOption::Integer(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Specify the target rank".to_owned(),
                            name: "rank".to_owned(),
                            required: true,
                        }),
                        CommandOption::String(ChoiceCommandOptionData {
                            choices: super::mode_choices(),
                            description: "Specify a gamemode".to_owned(),
                            name: "mode".to_owned(),
                            required: false,
                        }),
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
            ],
            required: false,
        }),
    ];

    SlashCommandBuilder::new("reach", description)
        .options(options)
        .build()
}
