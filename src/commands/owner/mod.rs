mod add_bg;
mod add_country;
mod cache;
mod change_game;
mod tracking_cooldown;
mod tracking_interval;
mod tracking_stats;
mod tracking_toggle;

pub use add_bg::*;
pub use add_country::*;
pub use cache::*;
pub use change_game::*;
pub use tracking_cooldown::*;
pub use tracking_interval::*;
pub use tracking_stats::*;
pub use tracking_toggle::*;

use crate::{
    util::{constants::common_literals::NAME, ApplicationCommandExt, CountryCode},
    BotResult, Context, Error,
};

use std::sync::Arc;
use twilight_model::application::interaction::{
    application_command::CommandDataOption, ApplicationCommand,
};

use super::{MyCommand, MyCommandOption};

enum OwnerCommandKind {
    AddCountry { code: CountryCode, country: String },
    Cache,
    ChangeGame(String),
    TrackingCooldown(f32),
    TrackingInterval(i64),
    TrackingStats,
    TrackingToggle,
}

const OWNER: &str = "owner";
const OWNER_ADD_COUNTRY: &str = "owner add_country";
const OWNER_CHANGE_GAME: &str = "owner change_game";
const OWNER_TRACKING: &str = "owner tracking";
const OWNER_TRACKING_COOLDOWN: &str = "owner tracking cooldown";
const OWNER_TRACKING_INTERVAL: &str = "owner tracking interval";

impl OwnerCommandKind {
    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!(OWNER, string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!(OWNER, integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(OWNER, boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
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
                                        NAME => country = Some(value),
                                        _ => bail_cmd_option!(OWNER_ADD_COUNTRY, string, name),
                                    }
                                }
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!(OWNER_ADD_COUNTRY, integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!(OWNER_ADD_COUNTRY, boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!(OWNER_ADD_COUNTRY, subcommand, name)
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
                                    _ => bail_cmd_option!(OWNER_CHANGE_GAME, string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!(OWNER_CHANGE_GAME, integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!(OWNER_CHANGE_GAME, boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!(OWNER_CHANGE_GAME, subcommand, name)
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
                                    bail_cmd_option!(OWNER_TRACKING, string, name)
                                }
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!(OWNER_TRACKING, integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!(OWNER_TRACKING, boolean, name)
                                }
                                CommandDataOption::SubCommand { name, options } => {
                                    match name.as_str() {
                                        "cooldown" => {
                                            let mut number = None;

                                            for option in options {
                                                match option {
                                                    CommandDataOption::String { name, value } => {
                                                        match name.as_str() {
                                                            "number" => number = value.parse().ok(),
                                                            _ => bail_cmd_option!(
                                                                OWNER_TRACKING_COOLDOWN,
                                                                string,
                                                                name
                                                            ),
                                                        }
                                                    }
                                                    CommandDataOption::Integer { name, .. } => {
                                                        bail_cmd_option!(
                                                            OWNER_TRACKING_COOLDOWN,
                                                            integer,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::Boolean { name, .. } => {
                                                        bail_cmd_option!(
                                                            OWNER_TRACKING_COOLDOWN,
                                                            boolean,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::SubCommand {
                                                        name, ..
                                                    } => bail_cmd_option!(
                                                        OWNER_TRACKING_COOLDOWN,
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
                                                            OWNER_TRACKING_INTERVAL,
                                                            string,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::Integer { name, value } => {
                                                        match name.as_str() {
                                                            "number" => number = Some(value.max(0)),
                                                            _ => bail_cmd_option!(
                                                                OWNER_TRACKING_INTERVAL,
                                                                integer,
                                                                name
                                                            ),
                                                        }
                                                    }
                                                    CommandDataOption::Boolean { name, .. } => {
                                                        bail_cmd_option!(
                                                            OWNER_TRACKING_INTERVAL,
                                                            boolean,
                                                            name
                                                        )
                                                    }
                                                    CommandDataOption::SubCommand {
                                                        name, ..
                                                    } => bail_cmd_option!(
                                                        OWNER_TRACKING_INTERVAL,
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
                                        _ => bail_cmd_option!(OWNER_TRACKING, subcommand, name),
                                    }
                                }
                            }
                        }
                    }
                    _ => bail_cmd_option!(OWNER, subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }
}

pub async fn slash_owner(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match OwnerCommandKind::slash(&mut command)? {
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

fn subcommand_addcountry() -> MyCommandOption {
    let code =
        MyCommandOption::builder("code", "Specify the country code").string(Vec::new(), true);
    let name = MyCommandOption::builder(NAME, "Specify the country name").string(Vec::new(), true);

    MyCommandOption::builder("add_country", "Add a country for snipe commands")
        .subcommand(vec![code, name])
}

fn subcommand_cache() -> MyCommandOption {
    MyCommandOption::builder("cache", "Display stats about the internal cache")
        .subcommand(Vec::new())
}

fn subcommand_changegame() -> MyCommandOption {
    let game = MyCommandOption::builder("game", "Specify the game name").string(Vec::new(), true);

    MyCommandOption::builder("change_game", "Change the game the bot is playing")
        .subcommand(vec![game])
}

fn subcommand_tracking() -> MyCommandOption {
    let number_description = "Specify the cooldown milliseconds, defaults to 5000.0";

    // TODO: Number variant
    let number = MyCommandOption::builder("number", number_description).integer(Vec::new(), false);

    let cooldown = MyCommandOption::builder("cooldown", "Adjust the tracking cooldown")
        .subcommand(vec![number]);

    let number =
        MyCommandOption::builder("number", "Specify the interval seconds, defaults to 7200")
            .integer(Vec::new(), false);

    let interval = MyCommandOption::builder("interval", "Adjust the tracking interval")
        .subcommand(vec![number]);

    let stats = MyCommandOption::builder("stats", "Display tracking stats").subcommand(Vec::new());

    let toggle =
        MyCommandOption::builder("toggle", "Enable or disable tracking").subcommand(Vec::new());

    MyCommandOption::builder("tracking", "Stuff about osu! tracking")
        .subcommandgroup(vec![cooldown, interval, stats, toggle])
}

pub fn define_owner() -> MyCommand {
    let options = vec![
        subcommand_addcountry(),
        subcommand_cache(),
        subcommand_changegame(),
        subcommand_tracking(),
    ];

    MyCommand::new(OWNER, "You won't be able to use this :^)").options(options)
}
