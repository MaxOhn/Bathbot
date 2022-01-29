mod add_bg;
mod add_country;
mod cache;
mod change_game;
mod tracking_cooldown;
mod tracking_interval;
mod tracking_stats;
mod tracking_toggle;

use std::sync::Arc;

use twilight_model::application::interaction::{
    application_command::{CommandDataOption, CommandOptionValue},
    ApplicationCommand,
};

use crate::{
    util::{constants::common_literals::NAME, CountryCode},
    BotResult, Context, Error,
};

pub use self::{
    add_bg::*, add_country::*, cache::*, change_game::*, tracking_cooldown::*,
    tracking_interval::*, tracking_stats::*, tracking_toggle::*,
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

impl OwnerCommandKind {
    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        command
            .data
            .options
            .pop()
            .and_then(|option| match option.value {
                CommandOptionValue::SubCommand(mut options) => match option.name.as_str() {
                    "add_country" => Self::slash_add_country(options),
                    "cache" => Some(Self::Cache),
                    "change_game" => {
                        let option = options.pop()?;

                        match (option.value, option.name.as_str()) {
                            (CommandOptionValue::String(value), "game") => {
                                Some(Self::ChangeGame(value))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                },
                CommandOptionValue::SubCommandGroup(options) => Self::slash_tracking(options),
                _ => None,
            })
            .ok_or(Error::InvalidCommandOptions)
    }

    fn slash_add_country(options: Vec<CommandDataOption>) -> Option<Self> {
        let mut code = None;
        let mut country = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(mut value) => match option.name.as_str() {
                    "code" => {
                        value.make_ascii_uppercase();
                        code = Some(value.into());
                    }
                    NAME => country = Some(value),
                    _ => return None,
                },
                _ => return None,
            }
        }

        let code = code?;
        let country = country?;

        Some(Self::AddCountry { code, country })
    }

    fn slash_tracking(options: Vec<CommandDataOption>) -> Option<Self> {
        options.first().and_then(|option| match &option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "cooldown" => options
                    .first()
                    .and_then(|option| {
                        (option.name == "number").then(|| match option.value {
                            CommandOptionValue::Number(value) => Some(value.0 as f32),
                            _ => None,
                        })
                    })
                    .flatten()
                    .map(Self::TrackingCooldown),
                "interval" => options
                    .first()
                    .and_then(|option| {
                        (option.name == "number").then(|| match option.value {
                            CommandOptionValue::Integer(value) => Some(value.max(0)),
                            _ => None,
                        })
                    })
                    .flatten()
                    .map(Self::TrackingInterval),
                "stats" => Some(Self::TrackingStats),
                "toggle" => Some(Self::TrackingToggle),
                _ => None,
            },
            _ => None,
        })
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
    let number = MyCommandOption::builder("number", number_description).number(Vec::new(), false);

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

    MyCommand::new("owner", "You won't be able to use this :^)").options(options)
}
