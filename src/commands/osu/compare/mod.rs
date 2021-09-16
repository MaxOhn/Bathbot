mod common;
mod most_played;
mod profile;
mod score;

pub use common::*;
pub use most_played::*;
pub use profile::*;
pub use score::*;

use super::{
    prepare_score, request_user, require_link, MinMaxAvgBasic, MinMaxAvgF32, MinMaxAvgU32,
};

use crate::{
    util::{matcher, ApplicationCommandExt, MessageExt},
    Args, BotResult, Context, Error, Name,
};

use rosu_v2::prelude::GameMode;
use std::{borrow::Cow, sync::Arc};
use twilight_model::{
    application::{
        command::{
            BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption,
            OptionsCommandOptionData,
        },
        interaction::{application_command::CommandDataOption, ApplicationCommand},
    },
    id::UserId,
};

const AT_LEAST_ONE: &str = "You need to specify at least one osu username. \
    If you're not linked, you must specify two names.";

struct TripleArgs {
    name1: Option<Name>,
    name2: Name,
    name3: Option<Name>,
    mode: GameMode,
}

impl TripleArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
        mode: Option<GameMode>,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let name1 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                    Some(name) => name,
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Ok(Err(content.into()));
                    }
                },
                None => arg.into(),
            },
            None => return Ok(Err(AT_LEAST_ONE.into())),
        };

        let mode = mode.unwrap_or(GameMode::STD);

        let name2 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                    Some(name) => name,
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Ok(Err(content.into()));
                    }
                },
                None => arg.into(),
            },
            None => {
                return Ok(Ok(Self {
                    name1: ctx.user_config(author_id).await?.osu_username,
                    name2: name1,
                    name3: None,
                    mode,
                }))
            }
        };

        let name3 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                    Some(name) => Some(name),
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Ok(Err(content.into()));
                    }
                },
                None => Some(arg.into()),
            },
            None => None,
        };

        let args = Self {
            name1: Some(name1),
            name2,
            name3,
            mode,
        };

        Ok(Ok(args))
    }

    async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let mut name1 = None;
        let mut name2 = None;
        let mut name3 = None;
        let mut mode = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "mode" => mode = parse_mode_option!(value, "compare top/mostplayed"),
                    "name1" => name1 = Some(value.into()),
                    "name2" => name2 = Some(value.into()),
                    "name3" => name3 = Some(value.into()),
                    "discord1" => match value.parse() {
                        Ok(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                            Some(name) => name1 = Some(name),
                            None => {
                                let content = format!("<@{}> is not linked to an osu profile", id);

                                return Ok(Err(content.into()));
                            }
                        },
                        Err(_) => {
                            bail_cmd_option!("compare top/mostplayed discord1", string, value)
                        }
                    },
                    "discord2" => match value.parse() {
                        Ok(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                            Some(name) => name2 = Some(name),
                            None => {
                                let content = format!("<@{}> is not linked to an osu profile", id);

                                return Ok(Err(content.into()));
                            }
                        },
                        Err(_) => {
                            bail_cmd_option!("compare top/mostplayed discord2", string, value)
                        }
                    },
                    "discord3" => match value.parse() {
                        Ok(id) => match ctx.user_config(UserId(id)).await?.osu_username {
                            Some(name) => name3 = Some(name),
                            None => {
                                let content = format!("<@{}> is not linked to an osu profile", id);

                                return Ok(Err(content.into()));
                            }
                        },
                        Err(_) => {
                            bail_cmd_option!("compare top/mostplayed discord3", string, value)
                        }
                    },
                    _ => bail_cmd_option!("compare top/mostplayed", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("compare top/mostplayed", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("compare top/mostplayed", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("compare top/mostplayed", subcommand, name)
                }
            }
        }

        let (name1, name2, name3) = match (name1, name2, name3) {
            (None, Some(name2), Some(name3)) => (Some(name2), name3, None),
            (name1, Some(name2), name3) => (name1, name2, name3),
            (Some(name1), None, Some(name3)) => (Some(name1), name3, None),
            (Some(name), None, None) => (None, name, None),
            (None, None, Some(name)) => (None, name, None),
            (None, None, None) => return Ok(Err(AT_LEAST_ONE.into())),
        };

        let name1 = match name1 {
            Some(name) => Some(name),
            None => ctx.user_config(author_id).await?.osu_username,
        };

        let args = TripleArgs {
            name1,
            name2,
            name3,
            mode: mode.unwrap_or(GameMode::STD),
        };

        Ok(Ok(args))
    }
}

enum CompareCommandKind {
    Score(ScoreArgs),
    Profile(ProfileArgs),
    Top(TripleArgs),
    Mostplayed(TripleArgs),
}

impl CompareCommandKind {
    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let author_id = command.user_id()?;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => bail_cmd_option!("compare", string, name),
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("compare", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("compare", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "score" => match ScoreArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(Self::Score(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "profile" => match ProfileArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(CompareCommandKind::Profile(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "top" => match TripleArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(CompareCommandKind::Top(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "mostplayed" => match TripleArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(CompareCommandKind::Mostplayed(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => bail_cmd_option!("compare", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }
}

pub async fn slash_compare(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match CompareCommandKind::slash(&ctx, &mut command).await? {
        Ok(CompareCommandKind::Score(args)) => _compare(ctx, command.into(), args).await,
        Ok(CompareCommandKind::Profile(args)) => _profilecompare(ctx, command.into(), args).await,
        Ok(CompareCommandKind::Top(args)) => _common(ctx, command.into(), args).await,
        Ok(CompareCommandKind::Mostplayed(args)) => {
            _mostplayedcommon(ctx, command.into(), args).await
        }
        Err(msg) => command.error(&ctx, msg).await,
    }
}

pub fn slash_compare_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "compare".to_owned(),
        default_permission: None,
        description: "Compare a score, top scores, or profiles".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Compare a score".to_owned(),
                name: "score".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a username".to_owned(),
                        name: "name".to_owned(),
                        required: false,
                    }),
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a map url or map id".to_owned(),
                        name: "map".to_owned(),
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
                description: "Compare two profiles".to_owned(),
                name: "profile".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify the gamemode".to_owned(),
                        name: "mode".to_owned(),
                        required: false,
                    }),
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
                description: "Compare common top scores".to_owned(),
                name: "top".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify the gamemode".to_owned(),
                        name: "mode".to_owned(),
                        required: false,
                    }),
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
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a username".to_owned(),
                        name: "name3".to_owned(),
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
                    CommandOption::User(BaseCommandOptionData {
                        description: "Specify a linked discord user".to_owned(),
                        name: "discord3".to_owned(),
                        required: false,
                    }),
                ],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Compare most played maps".to_owned(),
                name: "mostplayed".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: super::mode_choices(),
                        description: "Specify the gamemode".to_owned(),
                        name: "mode".to_owned(),
                        required: false,
                    }),
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
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify a username".to_owned(),
                        name: "name3".to_owned(),
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
                    CommandOption::User(BaseCommandOptionData {
                        description: "Specify a linked discord user".to_owned(),
                        name: "discord3".to_owned(),
                        required: false,
                    }),
                ],
                required: false,
            }),
        ],
    }
}
