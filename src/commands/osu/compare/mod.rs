mod common;
mod compare;
mod most_played;
mod profile;

pub use common::*;
pub use compare::*;
pub use most_played::*;
pub use profile::*;

use super::{
    prepare_score, request_user, require_link, MinMaxAvgBasic, MinMaxAvgF32, MinMaxAvgU32,
};

use crate::{
    util::{matcher, ApplicationCommandExt, MessageExt},
    Args, BotResult, Context, Error, Name,
};

use rosu_v2::prelude::GameMode;
use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::{
        BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption,
        OptionsCommandOptionData,
    },
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

const AT_LEAST_ONE: &str = "You need to specify at least one osu username. \
    If you're not linked, you must specify two names.";

struct ProfileArgs {
    name1: Option<Name>,
    name2: Name,
    mode: GameMode,
}

impl ProfileArgs {
    fn args(ctx: &Context, args: &mut Args, mode: GameMode) -> Result<Self, Cow<'static, str>> {
        let name2 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.get_link(id) {
                    Some(name) => name,
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Err(content.into());
                    }
                },
                None => arg.into(),
            },
            None => return Err(AT_LEAST_ONE.into()),
        };

        let args = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.get_link(id) {
                    Some(name) => Self {
                        name1: Some(name2),
                        name2: name,
                        mode,
                    },
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Err(content.into());
                    }
                },
                None => Self {
                    name1: Some(name2),
                    name2: arg.into(),
                    mode,
                },
            },
            None => Self {
                name1: None,
                name2,
                mode,
            },
        };

        Ok(args)
    }
}

struct TripleArgs {
    name1: Option<Name>,
    name2: Name,
    name3: Option<Name>,
    mode: GameMode,
}

impl TripleArgs {
    fn args(
        ctx: &Context,
        args: &mut Args,
        mode: Option<GameMode>,
    ) -> Result<Self, Cow<'static, str>> {
        let name1 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.get_link(id) {
                    Some(name) => name,
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Err(content.into());
                    }
                },
                None => arg.into(),
            },
            None => return Err(AT_LEAST_ONE.into()),
        };

        let mode = mode.unwrap_or(GameMode::STD);

        let name2 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.get_link(id) {
                    Some(name) => name,
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Err(content.into());
                    }
                },
                None => arg.into(),
            },
            None => {
                return Ok(Self {
                    name1: None,
                    name2: name1,
                    name3: None,
                    mode,
                })
            }
        };

        let name3 = match args.next() {
            Some(arg) => match matcher::get_mention_user(arg) {
                Some(id) => match ctx.get_link(id) {
                    Some(name) => Some(name),
                    None => {
                        let content = format!("<@{}> is not linked to an osu profile", id);

                        return Err(content.into());
                    }
                },
                None => Some(arg.into()),
            },
            None => None,
        };

        Ok(Self {
            name1: Some(name1),
            name2,
            name3,
            mode,
        })
    }
}

enum CompareCommandKind {
    Score(ScoreArgs),
    Profile(ProfileArgs),
    Top(TripleArgs),
    Mostplayed(TripleArgs),
}

impl CompareCommandKind {
    fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
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
                    "score" => match ScoreArgs::slash(ctx, options)? {
                        Ok(args) => kind = Some(Self::Score(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "profile" => {
                        let mut name1 = None;
                        let mut name2 = None;
                        let mut mode = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "mode" => mode = parse_mode_option!(value, "compare profile"),
                                    "name1" => name1 = Some(value.into()),
                                    "name2" => name2 = Some(value.into()),
                                    "discord1" => match value.parse() {
                                        Ok(id) => match ctx.get_link(id) {
                                            Some(name) => name1 = Some(name),
                                            None => {
                                                let content = format!(
                                                    "<@{}> is not linked to an osu profile",
                                                    id
                                                );

                                                return Ok(Err(content.into()));
                                            }
                                        },
                                        Err(_) => bail_cmd_option!(
                                            "compare profile discord1",
                                            string,
                                            value
                                        ),
                                    },
                                    "discord2" => match value.parse() {
                                        Ok(id) => match ctx.get_link(id) {
                                            Some(name) => name2 = Some(name),
                                            None => {
                                                let content = format!(
                                                    "<@{}> is not linked to an osu profile",
                                                    id
                                                );

                                                return Ok(Err(content.into()));
                                            }
                                        },
                                        Err(_) => bail_cmd_option!(
                                            "compare profile discord2",
                                            string,
                                            value
                                        ),
                                    },
                                    _ => bail_cmd_option!("compare profile", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("compare profile", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("compare profile", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("compare profile", subcommand, name)
                                }
                            }
                        }

                        let (name1, name2) = match (name1, name2) {
                            (name1, Some(name)) => (name1, name),
                            (Some(name), None) => (None, name),
                            (None, None) => return Ok(Err(AT_LEAST_ONE.into())),
                        };

                        let mode = mode.unwrap_or(GameMode::STD);
                        let args = ProfileArgs { name1, name2, mode };
                        kind = Some(CompareCommandKind::Profile(args));
                    }
                    "top" => {
                        let mut name1 = None;
                        let mut name2 = None;
                        let mut name3 = None;
                        let mut mode = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "mode" => mode = parse_mode_option!(value, "compare top"),
                                    "name1" => name1 = Some(value.into()),
                                    "name2" => name2 = Some(value.into()),
                                    "name3" => name3 = Some(value.into()),
                                    "discord1" => match value.parse() {
                                        Ok(id) => match ctx.get_link(id) {
                                            Some(name) => name1 = Some(name),
                                            None => {
                                                let content = format!(
                                                    "<@{}> is not linked to an osu profile",
                                                    id
                                                );

                                                return Ok(Err(content.into()));
                                            }
                                        },
                                        Err(_) => {
                                            bail_cmd_option!("compare top discord1", string, value)
                                        }
                                    },
                                    "discord2" => match value.parse() {
                                        Ok(id) => match ctx.get_link(id) {
                                            Some(name) => name2 = Some(name),
                                            None => {
                                                let content = format!(
                                                    "<@{}> is not linked to an osu profile",
                                                    id
                                                );

                                                return Ok(Err(content.into()));
                                            }
                                        },
                                        Err(_) => {
                                            bail_cmd_option!("compare top discord2", string, value)
                                        }
                                    },
                                    "discord3" => match value.parse() {
                                        Ok(id) => match ctx.get_link(id) {
                                            Some(name) => name3 = Some(name),
                                            None => {
                                                let content = format!(
                                                    "<@{}> is not linked to an osu profile",
                                                    id
                                                );

                                                return Ok(Err(content.into()));
                                            }
                                        },
                                        Err(_) => {
                                            bail_cmd_option!("compare top discord3", string, value)
                                        }
                                    },
                                    _ => bail_cmd_option!("compare top", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("compare top", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("compare top", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("compare top", subcommand, name)
                                }
                            }
                        }

                        let (name1, name2, name3) = match (name1, name2, name3) {
                            (name1, Some(name), name3) => (name1, name, name3),
                            (Some(name), None, name3) => (None, name, name3),
                            (None, None, Some(name)) => (None, name, None),
                            (None, None, None) => return Ok(Err(AT_LEAST_ONE.into())),
                        };

                        let args = TripleArgs {
                            name1,
                            name2,
                            name3,
                            mode: mode.unwrap_or(GameMode::STD),
                        };

                        kind = Some(CompareCommandKind::Top(args));
                    }
                    "mostplayed" => {
                        let mut name1 = None;
                        let mut name2 = None;
                        let mut name3 = None;
                        let mut mode = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "mode" => {
                                        mode = parse_mode_option!(value, "compare mostplayed")
                                    }
                                    "name1" => name1 = Some(value.into()),
                                    "name2" => name2 = Some(value.into()),
                                    "name3" => name3 = Some(value.into()),
                                    "discord1" => match value.parse() {
                                        Ok(id) => match ctx.get_link(id) {
                                            Some(name) => name1 = Some(name),
                                            None => {
                                                let content = format!(
                                                    "<@{}> is not linked to an osu profile",
                                                    id
                                                );

                                                return Ok(Err(content.into()));
                                            }
                                        },
                                        Err(_) => {
                                            bail_cmd_option!(
                                                "compare mostplayed discord1",
                                                string,
                                                value
                                            )
                                        }
                                    },
                                    "discord2" => match value.parse() {
                                        Ok(id) => match ctx.get_link(id) {
                                            Some(name) => name2 = Some(name),
                                            None => {
                                                let content = format!(
                                                    "<@{}> is not linked to an osu profile",
                                                    id
                                                );

                                                return Ok(Err(content.into()));
                                            }
                                        },
                                        Err(_) => {
                                            bail_cmd_option!(
                                                "compare mostplayed discord2",
                                                string,
                                                value
                                            )
                                        }
                                    },
                                    "discord3" => match value.parse() {
                                        Ok(id) => match ctx.get_link(id) {
                                            Some(name) => name3 = Some(name),
                                            None => {
                                                let content = format!(
                                                    "<@{}> is not linked to an osu profile",
                                                    id
                                                );

                                                return Ok(Err(content.into()));
                                            }
                                        },
                                        Err(_) => {
                                            bail_cmd_option!(
                                                "compare mostplayed discord3",
                                                string,
                                                value
                                            )
                                        }
                                    },
                                    _ => bail_cmd_option!("compare mostplayed", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("compare mostplayed", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("compare mostplayed", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("compare mostplayed", subcommand, name)
                                }
                            }
                        }

                        let (name1, name2, name3) = match (name1, name2, name3) {
                            (name1, Some(name), name3) => (name1, name, name3),
                            (Some(name), None, name3) => (None, name, name3),
                            (None, None, Some(name)) => (None, name, None),
                            (None, None, None) => return Ok(Err(AT_LEAST_ONE.into())),
                        };

                        let args = TripleArgs {
                            name1,
                            name2,
                            name3,
                            mode: mode.unwrap_or(GameMode::STD),
                        };

                        kind = Some(CompareCommandKind::Mostplayed(args));
                    }
                    _ => bail_cmd_option!("compare", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
    }
}

pub async fn slash_compare(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match CompareCommandKind::slash(&ctx, &mut command)? {
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
