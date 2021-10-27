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

use std::{borrow::Cow, sync::Arc};

use twilight_model::application::interaction::{
    application_command::CommandDataOption, ApplicationCommand,
};

use crate::{
    commands::{
        osu::{option_discord, option_name},
        MyCommand, MyCommandOption,
    },
    database::OsuData,
    util::{
        constants::common_literals::{DISCORD, INDEX, NAME},
        ApplicationCommandExt, InteractionExt, MessageExt,
    },
    BotResult, Context, Error, 
};

use super::{request_user, require_link};

enum MedalCommandKind {
    Common(CommonArgs),
    Medal(String),
    Missing(Option<Username>),
    Recent(RecentArgs),
    Stats(Option<Username>),
}

const MEDAL: &str = "medal";
const MEDAL_INFO: &str = "medal info";
const MEDAL_STATS: &str = "medal stats";
const MEDAL_MISSING: &str = "medal missing";

impl MedalCommandKind {
    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let author_id = command.user_id()?;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => bail_cmd_option!(MEDAL, string, name),
                CommandDataOption::Integer { name, .. } => bail_cmd_option!(MEDAL, integer, name),
                CommandDataOption::Boolean { name, .. } => bail_cmd_option!(MEDAL, boolean, name),
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "common" => match CommonArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(Self::Common(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    "info" => {
                        let mut medal_name = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    NAME => medal_name = Some(value),
                                    _ => bail_cmd_option!(MEDAL_INFO, string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!(MEDAL_INFO, integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!(MEDAL_INFO, boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!(MEDAL_INFO, subcommand, name)
                                }
                            }
                        }

                        let name = medal_name.ok_or(Error::InvalidCommandOptions)?;
                        kind = Some(MedalCommandKind::Medal(name));
                    }
                    "stats" => {
                        let mut username = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    NAME => username = Some(value.into()),
                                    DISCORD => {
                                        username =
                                            Some(parse_discord_option!(ctx, value, "medal stats"))
                                    }
                                    _ => bail_cmd_option!(MEDAL_STATS, string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!(MEDAL_STATS, integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!(MEDAL_STATS, boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!(MEDAL_STATS, subcommand, name)
                                }
                            }
                        }

                        let osu = match username {
                            Some(osu) => Some(osu),
                            None => ctx.psql().get_user_osu(author_id).await?,
                        };

                        kind = Some(MedalCommandKind::Stats(osu.map(OsuData::into_username)));
                    }
                    "missing" => {
                        let mut username = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    NAME => username = Some(value.into()),
                                    DISCORD => {
                                        username =
                                            Some(parse_discord_option!(ctx, value, "medal missing"))
                                    }
                                    _ => bail_cmd_option!(MEDAL_MISSING, string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!(MEDAL_MISSING, integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!(MEDAL_MISSING, boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!(MEDAL_MISSING, subcommand, name)
                                }
                            }
                        }

                        let osu = match username {
                            Some(osu) => Some(osu),
                            None => ctx.psql().get_user_osu(author_id).await?,
                        };

                        kind = Some(MedalCommandKind::Missing(osu.map(OsuData::into_username)));
                    }
                    "recent" => match RecentArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(Self::Recent(args)),
                        Err(content) => return Ok(Err(content.into())),
                    },
                    _ => bail_cmd_option!(MEDAL, subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
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

    MyCommand::new(MEDAL, "Info about a medal or users' medal progress")
        .help(help)
        .options(vec![common, info, missing, recent, stats])
}
