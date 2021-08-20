mod counts;
mod globals;
mod list;

pub use counts::*;
pub use globals::*;
pub use list::*;

use super::{get_globals_count, request_user, require_link};

use crate::{
    custom_client::OsuStatsListParams,
    util::{ApplicationCommandExt, MessageExt},
    BotResult, Context, Error,
};

use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption,
        CommandOptionChoice, OptionsCommandOptionData,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

enum OsustatsCommandKind {
    Count(CountArgs),
    Players(OsuStatsListParams),
    Scores(ScoresArgs),
}

impl OsustatsCommandKind {
    fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!("osustats", string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("osustats", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("osustats", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "count" => match CountArgs::slash(ctx, options)? {
                        Ok(args) => kind = Some(Self::Count(args)),
                        Err(content) => return Ok(Err(content.into())),
                    },
                    "players" => match OsuStatsListParams::slash(options)? {
                        Ok(args) => kind = Some(Self::Players(args)),
                        Err(content) => return Ok(Err(content.into())),
                    },
                    "scores" => match ScoresArgs::slash(ctx, options)? {
                        Ok(args) => kind = Some(Self::Scores(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => bail_cmd_option!("osustats", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }
}

pub async fn slash_osustats(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match OsustatsCommandKind::slash(&ctx, &mut command)? {
        Ok(OsustatsCommandKind::Count(args)) => _count(ctx, command.into(), args).await,
        Ok(OsustatsCommandKind::Players(args)) => _players(ctx, command.into(), args).await,
        Ok(OsustatsCommandKind::Scores(args)) => _scores(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

fn _slash_osustats_scores() -> Vec<CommandOption> {
    vec![
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
            choices: vec![
                CommandOptionChoice::String {
                    name: "accuracy".to_owned(),
                    value: "acc".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "combo".to_owned(),
                    value: "combo".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "misses".to_owned(),
                    value: "misses".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "pp".to_owned(),
                    value: "pp".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "rank".to_owned(),
                    value: "rank".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "score".to_owned(),
                    value: "score".to_owned(),
                },
                CommandOptionChoice::String {
                    name: "score date".to_owned(),
                    value: "date".to_owned(),
                },
            ],
            description: "Choose how the scores should be ordered".to_owned(),
            name: "sort".to_owned(),
            required: false,
        }),
        CommandOption::String(ChoiceCommandOptionData {
            choices: vec![],
            description:
                "Specify mods (`+mods` for included, `+mods!` for exact, `-mods!` for excluded)"
                    .to_owned(),
            name: "mods".to_owned(),
            required: false,
        }),
        CommandOption::Integer(ChoiceCommandOptionData {
            choices: vec![],
            description: "Specify a min rank between 1 and 100".to_owned(),
            name: "min_rank".to_owned(),
            required: false,
        }),
        CommandOption::Integer(ChoiceCommandOptionData {
            choices: vec![],
            description: "Specify a max rank between 1 and 100".to_owned(),
            name: "max_rank".to_owned(),
            required: false,
        }),
        // CommandOption::Number(ChoiceCommandOptionData {
        //     choices: vec![],
        //     description: "Specify a min accuracy".to_owned(),
        //     name: "min_acc".to_owned(),
        //     required: false,
        // }),
        // CommandOption::Number(ChoiceCommandOptionData {
        //     choices: vec![],
        //     description: "Specify a max accuracy".to_owned(),
        //     name: "max_acc".to_owned(),
        //     required: false,
        // }),
        CommandOption::Boolean(BaseCommandOptionData {
            description: "Reverse the resulting score list".to_owned(),
            name: "reverse".to_owned(),
            required: false,
        }),
        CommandOption::User(BaseCommandOptionData {
            description: "Specify a linked discord user".to_owned(),
            name: "discord".to_owned(),
            required: false,
        }),
    ]
}

pub fn slash_osustats_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "osustats".to_owned(),
        default_permission: None,
        description: "Stats about players' appearances in maps' leaderboards".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Count how often a user appears on top of maps' leaderboards"
                    .to_owned(),
                name: "count".to_owned(),
                options: vec![
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
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "National player leaderboard of global leaderboard counts".to_owned(),
                name: "players".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify a gamemode".to_owned(),
                        name: "mode".to_owned(),
                        required: false,
                    }),
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a country code".to_owned(),
                        name: "country".to_owned(),
                        required: false,
                    }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a min rank between 1 and 100".to_owned(),
                        name: "min_rank".to_owned(),
                        required: false,
                    }),
                    CommandOption::Integer(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a max rank between 1 and 100".to_owned(),
                        name: "max_rank".to_owned(),
                        required: false,
                    }),
                ],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "All scores of a player that are on a map's global leaderboard"
                    .to_owned(),
                name: "scores".to_owned(),
                options: _slash_osustats_scores(),
                required: false,
            }),
        ],
    }
}
