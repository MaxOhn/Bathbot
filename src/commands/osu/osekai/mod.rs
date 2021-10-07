mod medal_count;
mod rarity;
mod user_value;

use medal_count::medal_count;
use rarity::rarity;
use user_value::{count, pp};

use std::sync::Arc;

use twilight_model::application::interaction::{
    application_command::CommandDataOption, ApplicationCommand,
};

use crate::{
    commands::{MyCommand, MyCommandOption},
    custom_client::{
        Badges, LovedMapsets, RankedMapsets, Replays, StandardDeviation, Subscribers, TotalPp,
    },
    util::ApplicationCommandExt,
    BotResult, Context, Error,
};

use super::UserValue;

enum OsekaiCommandKind {
    Badges,
    LovedMapsets,
    MedalCount,
    RankedMapsets,
    Rarity,
    Replays,
    StandardDeviation,
    Subscribers,
    TotalPp,
}

const OSEKAI: &str = "osekai";

impl OsekaiCommandKind {
    async fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!(OSEKAI, string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!(OSEKAI, integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(OSEKAI, boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => match name.as_str() {
                    "badges" => kind = Some(OsekaiCommandKind::Badges),
                    "loved_mapsets" => kind = Some(OsekaiCommandKind::LovedMapsets),
                    "medal_count" => kind = Some(OsekaiCommandKind::MedalCount),
                    "ranked_mapsets" => kind = Some(OsekaiCommandKind::RankedMapsets),
                    "rarity" => kind = Some(OsekaiCommandKind::Rarity),
                    "replays" => kind = Some(OsekaiCommandKind::Replays),
                    "standard_deviation" => kind = Some(OsekaiCommandKind::StandardDeviation),
                    "subscribers" => kind = Some(OsekaiCommandKind::Subscribers),
                    "total_pp" => kind = Some(OsekaiCommandKind::TotalPp),
                    _ => bail_cmd_option!(OSEKAI, subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }
}

pub async fn slash_osekai(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match OsekaiCommandKind::slash(&mut command).await? {
        OsekaiCommandKind::Badges => count(ctx, command, Badges).await,
        OsekaiCommandKind::LovedMapsets => count(ctx, command, LovedMapsets).await,
        OsekaiCommandKind::MedalCount => medal_count(ctx, command).await,
        OsekaiCommandKind::RankedMapsets => count(ctx, command, RankedMapsets).await,
        OsekaiCommandKind::Rarity => rarity(ctx, command).await,
        OsekaiCommandKind::Replays => count(ctx, command, Replays).await,
        OsekaiCommandKind::StandardDeviation => pp(ctx, command, StandardDeviation).await,
        OsekaiCommandKind::Subscribers => count(ctx, command, Subscribers).await,
        OsekaiCommandKind::TotalPp => pp(ctx, command, TotalPp).await,
    }
}

pub fn define_osekai() -> MyCommand {
    let badges = MyCommandOption::builder("badges", "Who has the most profile badges?")
        .subcommand(Vec::new());

    let loved_mapsets =
        MyCommandOption::builder("loved_mapsets", "Who created the most loved mapsets?")
            .subcommand(Vec::new());

    let medal_count =
        MyCommandOption::builder("medal_count", "Who has the most medals?").subcommand(Vec::new());

    let ranked_mapsets =
        MyCommandOption::builder("ranked_mapsets", "Who created the most ranked mapsets?")
            .subcommand(Vec::new());

    let rarity =
        MyCommandOption::builder("rarity", "What are the rarest medals?").subcommand(Vec::new());

    let replays = MyCommandOption::builder("replays", "Who has the most replays watched?")
        .subcommand(Vec::new());

    let standard_deviation_description =
        "Who has the highest pp standard deviation across all modes?";

    let standard_deviation_help = "Who has the highest pp [standard deviation](https://en.wikipedia.org/wiki/Standard_deviation) across all modes?";

    let standard_deviation =
        MyCommandOption::builder("standard_deviation", standard_deviation_description)
            .help(standard_deviation_help)
            .subcommand(Vec::new());

    let subscribers_description = "Which mapper has the most subscribers?";

    let subscribers =
        MyCommandOption::builder("subscribers", subscribers_description).subcommand(Vec::new());

    let total_pp_description = "Who has the highest total pp in all modes combined?";

    let total_pp =
        MyCommandOption::builder("total_pp", total_pp_description).subcommand(Vec::new());

    let options = vec![
        badges,
        loved_mapsets,
        medal_count,
        ranked_mapsets,
        rarity,
        replays,
        standard_deviation,
        subscribers,
        total_pp,
    ];

    let help = "Various leaderboard stats. \
        All data is provided by [osekai](https://osekai.net/).";

    MyCommand::new(OSEKAI, "Various leaderboards provided by osekai")
        .help(help)
        .options(options)
}
