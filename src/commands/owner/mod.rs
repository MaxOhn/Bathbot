mod active_bg;
mod add_bg;
mod add_country;
mod cache;
mod change_game;
mod tracking_cooldown;
mod tracking_interval;
mod tracking_stats;
mod tracking_toggle;

pub use active_bg::*;
pub use add_bg::*;
pub use add_country::*;
pub use cache::*;
pub use change_game::*;
pub use tracking_cooldown::*;
pub use tracking_interval::*;
pub use tracking_stats::*;
pub use tracking_toggle::*;

use crate::{
    util::{ApplicationCommandExt, CountryCode},
    BotResult, Context, Error,
};

use std::sync::Arc;
use twilight_model::application::{
    command::{ChoiceCommandOptionData, Command, CommandOption, OptionsCommandOptionData},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

enum OwnerCommandKind {
    ActiveBg,
    AddCountry { code: CountryCode, country: String },
    Cache,
    ChangeGame(String),
    TrackingCooldown(f32),
    TrackingInterval(i64),
    TrackingStats,
    TrackingToggle,
}

impl OwnerCommandKind {
    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!("owner", string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("owner", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("owner", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "active_bg" => kind = Some(Self::ActiveBg),
                    "add_country" => {
                        let mut code = None;
                        let mut country = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, mut value } => {
                                    match name.as_str() {
                                        "code" => {
                                            value.make_ascii_uppercase();
                                            code = Some(value.into());
                                        }
                                        "name" => country = Some(value),
                                        _ => bail_cmd_option!("owner add_country", string, name),
                                    }
                                }
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("owner add_country", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("owner add_country", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("owner add_country", subcommand, name)
                                }
                            }
                        }

                        let code = code.ok_or(Error::InvalidCommandOptions)?;
                        let country = country.ok_or(Error::InvalidCommandOptions)?;
                        kind = Some(Self::AddCountry { code, country });
                    }
                    "cache" => kind = Some(Self::Cache),
                    "change_game" => {
                        let mut game = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match value.as_str() {
                                    "game" => game = Some(value),
                                    _ => bail_cmd_option!("owner change_game", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("owner change_game", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("owner change_game", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("owner change_game", subcommand, name)
                                }
                            }
                        }

                        let game = game.ok_or(Error::InvalidCommandOptions)?;
                        kind = Some(Self::ChangeGame(game));
                    }
                    "tracking" => {
                        for option in options {
                            match option {
                                CommandDataOption::String { name, .. } => {
                                    bail_cmd_option!("owner tracking", string, name)
                                }
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("owner tracking", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("owner tracking", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, options } => {
                                    match name.as_str() {
                                        "cooldown" => {
                                            let mut number = None;

                                            for option in options {
                                                match option {
                                                    CommandDataOption::String { name, .. } => {
                                                        bail_cmd_option!(
                                                            "owner tracking cooldown",
                                                            string,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::Integer { name, .. } => {
                                                        bail_cmd_option!(
                                                            "owner tracking cooldown",
                                                            integer,
                                                            name
                                                        )
                                                    }
                                                    // CommandDataOption::Number { name, value } => {
                                                    //     match name.as_str() {
                                                    //         "number" => number = Some(value),
                                                    //         _ => bail_cmd_option!(
                                                    //             "owner tracking cooldown",
                                                    //             number,
                                                    //             name
                                                    //         ),
                                                    //     }
                                                    // }
                                                    CommandDataOption::Boolean { name, .. } => {
                                                        bail_cmd_option!(
                                                            "owner tracking cooldown",
                                                            boolean,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::SubCommand {
                                                        name, ..
                                                    } => bail_cmd_option!(
                                                        "owner tracking cooldown",
                                                        subcommand,
                                                        name
                                                    ),
                                                }
                                            }

                                            let number =
                                                number.ok_or(Error::InvalidCommandOptions)?;
                                            kind = Some(Self::TrackingCooldown(number));
                                        }
                                        "interval" => {
                                            let mut number = None;

                                            for option in options {
                                                match option {
                                                    CommandDataOption::String { name, .. } => {
                                                        bail_cmd_option!(
                                                            "owner tracking interval",
                                                            string,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::Integer { name, value } => {
                                                        match name.as_str() {
                                                            "number" => number = Some(value.max(0)),
                                                            _ => bail_cmd_option!(
                                                                "owner tracking interval",
                                                                integer,
                                                                name
                                                            ),
                                                        }
                                                    }
                                                    CommandDataOption::Boolean { name, .. } => {
                                                        bail_cmd_option!(
                                                            "owner tracking interval",
                                                            boolean,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::SubCommand {
                                                        name, ..
                                                    } => bail_cmd_option!(
                                                        "owner tracking interval",
                                                        subcommand,
                                                        name
                                                    ),
                                                }
                                            }

                                            let number =
                                                number.ok_or(Error::InvalidCommandOptions)?;
                                            kind = Some(Self::TrackingInterval(number));
                                        }
                                        "stats" => kind = Some(Self::TrackingStats),
                                        "toggle" => kind = Some(Self::TrackingToggle),
                                        _ => bail_cmd_option!("owner tracking", subcommand, name),
                                    }
                                }
                            }
                        }
                    }
                    _ => bail_cmd_option!("owner", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }
}

pub async fn slash_owner(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match OwnerCommandKind::slash(&mut command)? {
        OwnerCommandKind::ActiveBg => activebg(ctx, command.into()).await,
        OwnerCommandKind::AddCountry { code, country } => {
            _addcountry(ctx, command.into(), code, country).await
        }
        OwnerCommandKind::Cache => cache(ctx, command.into()).await,
        OwnerCommandKind::ChangeGame(game) => _changegame(ctx, command.into(), game).await,
        OwnerCommandKind::TrackingCooldown(ms) => _trackingcooldown(ctx, command.into(), ms).await,
        OwnerCommandKind::TrackingInterval(seconds) => {
            _trackinginterval(ctx, command.into(), seconds).await
        }
        OwnerCommandKind::TrackingStats => trackingstats(ctx, command.into()).await,
        OwnerCommandKind::TrackingToggle => trackingtoggle(ctx, command.into()).await,
    }
}

pub fn slash_owner_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "owner".to_owned(),
        default_permission: None,
        description: "You won't be able to use this :^)".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Display which channels are currently playing the bg game".to_owned(),
                name: "active_bg".to_owned(),
                options: vec![],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Add a country for snipe commands".to_owned(),
                name: "add_country".to_owned(),
                options: vec![
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify the country code".to_owned(),
                        name: "code".to_owned(),
                        required: true,
                    }),
                    CommandOption::String(ChoiceCommandOptionData {
                        choices: vec![],
                        description: "Specify the country name".to_owned(),
                        name: "name".to_owned(),
                        required: true,
                    }),
                ],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Display stats about the internal cache".to_owned(),
                name: "cache".to_owned(),
                options: vec![],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Change the game the bot is playing".to_owned(),
                name: "change_game".to_owned(),
                options: vec![CommandOption::String(ChoiceCommandOptionData {
                    choices: vec![],
                    description: "Specify the game name, defaults to osu!".to_owned(),
                    name: "game".to_owned(),
                    required: true,
                })],
                required: false,
            }),
            CommandOption::SubCommandGroup(OptionsCommandOptionData {
                description: "Stuff about osu! tracking".to_owned(),
                name: "tracking".to_owned(),
                options: vec![
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "Adjust the tracking cooldown".to_owned(),
                        name: "cooldown".to_owned(),
                        options: vec![
                        //     CommandOption::Number(ChoiceCommandOptionData {
                        //     choices: vec![],
                        //     description: "Specify the cooldown milliseconds, defaults to 5000.0".to_owned(),
                        //     name: "number".to_owned(),
                        //     required: true,
                        // })
                        ],
                        required: false,
                    }),
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "Adjust the tracking interval".to_owned(),
                        name: "interval".to_owned(),
                        options: vec![CommandOption::Integer(ChoiceCommandOptionData {
                            choices: vec![],
                            description: "Specify the interval seconds, defaults to 7200"
                                .to_owned(),
                            name: "number".to_owned(),
                            required: true,
                        })],
                        required: false,
                    }),
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "Display tracking stats".to_owned(),
                        name: "stats".to_owned(),
                        options: vec![],
                        required: false,
                    }),
                    CommandOption::SubCommand(OptionsCommandOptionData {
                        description: "Enable or disable tracking".to_owned(),
                        name: "toggle".to_owned(),
                        options: vec![],
                        required: false,
                    }),
                ],
                required: false,
            }),
        ],
    }
}
