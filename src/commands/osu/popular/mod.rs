mod mappers;
mod maps;
mod mapsets;
mod mods;

use std::sync::Arc;

use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{application_command::CommandOptionValue, ApplicationCommand},
};

use crate::{
    commands::{MyCommand, MyCommandOption},
    core::Context,
    error::Error,
    BotResult,
};

pub use self::mapsets::MapsetEntry;

enum PopularCommandKind {
    Maps { pp: u32 },
    Mapsets,
    Mappers,
    Mods,
}

impl PopularCommandKind {
    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let option = command
            .data
            .options
            .pop()
            .ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "maps" => {
                    let mut pp = None;

                    for option in options {
                        match option.value {
                            CommandOptionValue::String(value) => match option.name.as_str() {
                                "pp" => {
                                    let value = value
                                        .split('_')
                                        .next()
                                        .map(str::parse)
                                        .map(Result::ok)
                                        .flatten()
                                        .ok_or(Error::InvalidCommandOptions)?;

                                    pp = Some(value);
                                }
                                _ => return Err(Error::InvalidCommandOptions),
                            },
                            _ => return Err(Error::InvalidCommandOptions),
                        }
                    }

                    let pp = pp.ok_or(Error::InvalidCommandOptions)?;

                    Ok(PopularCommandKind::Maps { pp })
                }
                "mapsets" => Ok(PopularCommandKind::Mapsets),
                "mods" => Ok(PopularCommandKind::Mods),
                "mappers" => Ok(PopularCommandKind::Mappers),
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }
}

pub async fn slash_popular(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match PopularCommandKind::slash(&mut command)? {
        PopularCommandKind::Maps { pp } => maps::maps_(ctx, command.into(), pp).await,
        PopularCommandKind::Mapsets => mapsets::mapsets_(ctx, command.into()).await,
        PopularCommandKind::Mappers => mappers::mappers_(ctx, command.into()).await,
        PopularCommandKind::Mods => mods::mods_(ctx, command.into()).await,
    }
}

fn subcommand_maps() -> MyCommandOption {
    let pp_choices = vec![
        CommandOptionChoice::String {
            name: "100-200pp".to_owned(),
            value: "100_200".to_owned(),
        },
        CommandOptionChoice::String {
            name: "200-300pp".to_owned(),
            value: "200_300".to_owned(),
        },
        CommandOptionChoice::String {
            name: "300-400pp".to_owned(),
            value: "300_400".to_owned(),
        },
        CommandOptionChoice::String {
            name: "400-500pp".to_owned(),
            value: "400_500".to_owned(),
        },
        CommandOptionChoice::String {
            name: "500-600pp".to_owned(),
            value: "500_600".to_owned(),
        },
        CommandOptionChoice::String {
            name: "600-700pp".to_owned(),
            value: "600_700".to_owned(),
        },
        CommandOptionChoice::String {
            name: "700-800pp".to_owned(),
            value: "700_800".to_owned(),
        },
        CommandOptionChoice::String {
            name: "800-900pp".to_owned(),
            value: "800_900".to_owned(),
        },
        CommandOptionChoice::String {
            name: "1000-1100pp".to_owned(),
            value: "1000_1100".to_owned(),
        },
        CommandOptionChoice::String {
            name: "1100-1200pp".to_owned(),
            value: "1100_1200".to_owned(),
        },
        CommandOptionChoice::String {
            name: "1200-1300pp".to_owned(),
            value: "1200_1300".to_owned(),
        },
    ];

    let pp = MyCommandOption::builder("pp", "Specify a pp range").string(pp_choices, true);

    MyCommandOption::builder("maps", "What are the most common maps per pp range?")
        .subcommand(vec![pp])
}

fn subcommand_mapsets() -> MyCommandOption {
    let description = "What mapsets appear the most in people's top100?";

    MyCommandOption::builder("mapsets", description).subcommand(Vec::new())
}

fn subcommand_mods() -> MyCommandOption {
    MyCommandOption::builder("mods", "What mods appear the most in people's top100?")
        .subcommand(Vec::new())
}

fn subcommand_mappers() -> MyCommandOption {
    let description = "What mappers' mapsets appear the most in people's top100?";

    MyCommandOption::builder("mappers", description).subcommand(Vec::new())
}

pub fn define_popular() -> MyCommand {
    let help = "Check out the most popular map(set)s, mods, or mappers.\n\
        All data is provided by [nzbasic](https://osu.ppy.sh/users/9008211)'s website [osutracker](https://osutracker.com/).";

    let options = vec![
        subcommand_maps(),
        subcommand_mapsets(),
        subcommand_mods(),
        subcommand_mappers(),
    ];

    let description = "Check out the most popular map(set)s, mods, or mappers";

    MyCommand::new("popular", description)
        .help(help)
        .options(options)
}
