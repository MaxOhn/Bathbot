mod common;
mod medal;
mod missing;
mod recent;
mod stats;

pub use common::*;
pub use medal::*;
pub use missing::*;
pub use recent::*;
use rosu_v2::prelude::Username;
pub use stats::*;

use std::sync::Arc;

use twilight_model::application::interaction::{
    application_command::{CommandDataOption, CommandOptionValue},
    ApplicationCommand,
};

use crate::{
    commands::{
        osu::{option_discord, option_name},
        parse_discord, DoubleResultCow, MyCommand, MyCommandOption,
    },
    database::OsuData,
    util::{
        constants::common_literals::{DISCORD, INDEX, NAME},
        InteractionExt, MessageExt,
    },
    BotResult, Context, Error,
};

use super::require_link;

enum MedalCommandKind {
    Common(CommonArgs),
    Medal(String),
    Missing(Option<Username>),
    Recent(RecentArgs),
    Stats(Option<Username>),
}

async fn parse_username(
    ctx: &Context,
    command: &ApplicationCommand,
    options: Vec<CommandDataOption>,
) -> DoubleResultCow<Option<Username>> {
    let mut osu = None;

    for option in options {
        match option.value {
            CommandOptionValue::String(value) => match option.name.as_str() {
                NAME => osu = Some(value.into()),
                _ => return Err(Error::InvalidCommandOptions),
            },
            CommandOptionValue::User(value) => match option.name.as_str() {
                DISCORD => match parse_discord(ctx, value).await? {
                    Ok(osu_) => osu = Some(osu_),
                    Err(content) => return Ok(Err(content)),
                },
                _ => return Err(Error::InvalidCommandOptions),
            },
            _ => return Err(Error::InvalidCommandOptions),
        }
    }

    let osu = match osu {
        Some(osu) => Some(osu),
        None => ctx.psql().get_user_osu(command.user_id()?).await?,
    };

    Ok(Ok(osu.map(OsuData::into_username)))
}

impl MedalCommandKind {
    fn slash_info(mut options: Vec<CommandDataOption>) -> BotResult<Self> {
        options
            .pop()
            .and_then(|option| (option.name == NAME).then(|| option.value))
            .and_then(|value| match value {
                CommandOptionValue::String(value) => Some(value),
                _ => None,
            })
            .map(Self::Medal)
            .ok_or(Error::InvalidCommandOptions)
    }

    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let option = command
            .data
            .options
            .pop()
            .ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "common" => match CommonArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(Self::Common(args))),
                    Err(content) => Ok(Err(content)),
                },
                "info" => Self::slash_info(options).map(Ok),
                "stats" => match parse_username(ctx, command, options).await? {
                    Ok(name) => Ok(Ok(Self::Stats(name))),
                    Err(content) => Ok(Err(content)),
                },
                "missing" => match parse_username(ctx, command, options).await? {
                    Ok(name) => Ok(Ok(Self::Missing(name))),
                    Err(content) => Ok(Err(content)),
                },
                "recent" => match RecentArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(Self::Recent(args))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
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

fn option_name_(n: u8) -> MyCommandOption {
    let mut name = option_name();

    name.name = match n {
        1 => "name1",
        2 => "name2",
        _ => unreachable!(),
    };

    name
}

fn option_discord_(n: u8) -> MyCommandOption {
    let mut discord = option_discord();

    discord.name = match n {
        1 => "discord1",
        2 => "discord2",
        _ => unreachable!(),
    };

    discord.help = if n == 1 {
        Some(
            "Instead of specifying an osu! username with the `name1` option, \
            you can use this `discord1` option to choose a discord user.\n\
            For it to work, the user must be linked to an osu! account i.e. they must have used \
            the `/link` or `/config` command to verify their account.",
        )
    } else {
        None
    };

    discord
}

pub fn define_medal() -> MyCommand {
    let name1 = option_name_(1);
    let name2 = option_name_(2);
    let discord1 = option_discord_(1);
    let discord2 = option_discord_(2);

    let common_description = "Compare which of the given users achieved medals first";

    let common = MyCommandOption::builder("common", common_description)
        .subcommand(vec![name1, name2, discord1, discord2]);

    let name_help = "Specify the name of a medal.\n\
        Upper- and lowercase does not matter but punctuation is important.";

    let name = MyCommandOption::builder(NAME, "Specify the name of a medal")
        .help(name_help)
        .string(Vec::new(), true);

    let info_help = "Display info about an osu! medal.\n\
        The solution, beatmaps, and comments are provided by [osekai](https://osekai.net/).";

    let info = MyCommandOption::builder("info", "Display info about an osu! medal")
        .help(info_help)
        .subcommand(vec![name]);

    let name = option_name();
    let discord = option_discord();

    let missing =
        MyCommandOption::builder("missing", "Display a list of medals that a user is missing")
            .subcommand(vec![name, discord]);

    let name = option_name();
    let discord = option_discord();

    let index = MyCommandOption::builder(INDEX, "Specify an index e.g. 1 = most recent")
        .integer(Vec::new(), false);

    let recent_help = "Display a recently acquired medal of a user.\n\
        The solution, beatmaps, and comments are provided by [osekai](https://osekai.net/).";

    let recent = MyCommandOption::builder("recent", "Display a recently acquired medal of a user")
        .help(recent_help)
        .subcommand(vec![name, index, discord]);

    let name = option_name();
    let discord = option_discord();

    let stats = MyCommandOption::builder("stats", "Display medal stats for a user")
        .subcommand(vec![name, discord]);

    let help = "Info about a medal or users' medal progress.\n\
        Check out [osekai](https://osekai.net/) for more info on medals.";

    MyCommand::new("medal", "Info about a medal or users' medal progress")
        .help(help)
        .options(vec![common, info, missing, recent, stats])
}
