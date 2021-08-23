mod bigger;
mod hint;
mod rankings;
mod start;
mod stop;
mod tags;

pub use bigger::*;
pub use hint::*;
pub use rankings::*;
pub use start::*;
pub use stop::*;
pub use tags::*;

use crate::{
    embeds::{BGHelpEmbed, EmbedData},
    util::{ApplicationCommandExt, MessageExt},
    BotResult, CommandData, Context, Error,
};

use rosu_v2::prelude::GameMode;
use std::sync::Arc;
use twilight_model::{
    application::{
        command::{
            ChoiceCommandOptionData, Command, CommandOption, CommandOptionChoice,
            OptionsCommandOptionData,
        },
        interaction::{application_command::CommandDataOption, ApplicationCommand},
    },
    channel::Reaction,
};

#[command]
#[short_desc("Play the background guessing game")]
#[long_desc(
    "Play the background guessing game.\n\
    Use this command without arguments to see the full help."
)]
#[aliases("bg")]
#[sub_commands(start, bigger, hint, stop, rankings)]
pub async fn backgroundgame(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, .. } => match args.next() {
            None | Some("help") => {
                let builder = BGHelpEmbed::new().into_builder().build().into();
                msg.create_message(&ctx, builder).await?;

                Ok(())
            }
            _ => {
                let prefix = ctx.config_first_prefix(msg.guild_id);

                let content = format!(
                    "That's not a valid subcommand. Check `{}bg` for more help.",
                    prefix
                );

                msg.error(&ctx, content).await
            }
        },
        CommandData::Interaction { command } => slash_backgroundgame(ctx, command).await,
    }
}

enum ReactionWrapper {
    Add(Reaction),
    Remove(Reaction),
}

impl ReactionWrapper {
    fn as_deref(&self) -> &Reaction {
        match self {
            Self::Add(r) | Self::Remove(r) => r,
        }
    }
}

enum GameCommandKind {
    Start { mode: GameMode },
    Skip,
    Bigger,
    Hint,
    Stop,
    Help,
    Leaderboard { global: bool },
}

impl GameCommandKind {
    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!("backgroundgame", string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("backgroundgame", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("backgroundgame", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "start" => {
                        let mut mode = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "mode" => {
                                        mode = parse_mode_option!(value, "backgroundgame start")
                                    }
                                    _ => bail_cmd_option!("backgroundgame start", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("backgroundgame start", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("backgroundgame start", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("backgroundgame start", subcommand, name)
                                }
                            }
                        }

                        let mode = mode.unwrap_or(GameMode::STD);
                        kind = Some(GameCommandKind::Start { mode });
                    }
                    "skip" => kind = Some(GameCommandKind::Skip),
                    "bigger" => kind = Some(GameCommandKind::Bigger),
                    "hint" => kind = Some(GameCommandKind::Hint),
                    "stop" => kind = Some(GameCommandKind::Stop),
                    "help" => kind = Some(GameCommandKind::Help),
                    "leaderboard" => {
                        let mut global = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "global" => match value.as_str() {
                                        "global" => global = Some(true),
                                        "server" => global = Some(false),
                                        _ => bail_cmd_option!(
                                            "backgroundgame start leaderboard",
                                            string,
                                            value
                                        ),
                                    },
                                    _ => bail_cmd_option!("backgroundgame start", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("backgroundgame start", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("backgroundgame start", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("backgroundgame start", subcommand, name)
                                }
                            }
                        }

                        let global = global.ok_or(Error::InvalidCommandOptions)?;
                        kind = Some(GameCommandKind::Leaderboard { global })
                    }
                    _ => bail_cmd_option!("backgroundgame", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }
}

pub async fn slash_backgroundgame(
    ctx: Arc<Context>,
    mut command: ApplicationCommand,
) -> BotResult<()> {
    match GameCommandKind::slash(&mut command)? {
        GameCommandKind::Start { mode } => _start(ctx, command.into(), mode).await,
        GameCommandKind::Skip => restart(&ctx, &command.into()).await.map(|_| ()),
        GameCommandKind::Bigger => bigger(ctx, command.into()).await,
        GameCommandKind::Hint => hint(ctx, command.into()).await,
        GameCommandKind::Stop => stop(ctx, command.into()).await,
        GameCommandKind::Help => {
            let builder = BGHelpEmbed::new().into_builder().build().into();
            command.create_message(&ctx, builder).await?;

            Ok(())
        }
        GameCommandKind::Leaderboard { global } => _rankings(ctx, command.into(), global).await,
    }
}

pub fn slash_backgroundgame_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "backgroundgame".to_owned(),
        default_permission: None,
        description: "Play the background guessing game".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Start a new game".to_owned(),
                name: "start".to_owned(),
                options: vec![CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![
                        CommandOptionChoice::String {
                            name: "osu".to_owned(),
                            value: "osu".to_owned(),
                        },
                        CommandOptionChoice::String {
                            name: "mania".to_owned(),
                            value: "mania".to_owned(),
                        },
                    ],
                    description: "Specify the gamemode".to_owned(),
                    name: "mode".to_owned(),
                    required: false,
                })],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Skip the current background".to_owned(),
                name: "skip".to_owned(),
                options: vec![],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Increase the size of the shown background".to_owned(),
                name: "bigger".to_owned(),
                options: vec![],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Get a hint".to_owned(),
                name: "hint".to_owned(),
                options: vec![],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Stop the game".to_owned(),
                name: "stop".to_owned(),
                options: vec![],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Display info how the game works".to_owned(),
                name: "help".to_owned(),
                options: vec![],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Display the leaderboard for correct guesses".to_owned(),
                name: "leaderboard".to_owned(),
                options: vec![CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![
                        CommandOptionChoice::String {
                            name: "global".to_owned(),
                            value: "global".to_owned(),
                        },
                        CommandOptionChoice::String {
                            name: "server".to_owned(),
                            value: "server".to_owned(),
                        },
                    ],
                    description: "Choose for global or server leaderboard".to_owned(),
                    name: "global".to_owned(),
                    required: true,
                })],
                required: false,
            }),
        ],
    }
}
