mod counts;
mod globals;
mod list;

pub use counts::*;
pub use globals::*;
pub use list::*;

use super::{get_globals_count, require_link};

use crate::{
    commands::{
        osu::{option_country, option_discord, option_mode, option_mods_explicit, option_name},
        DoubleResultCow, MyCommand, MyCommandOption,
    },
    custom_client::OsuStatsListParams,
    util::{
        constants::common_literals::{ACC, ACCURACY, COMBO, MISSES, RANK, REVERSE, SCORE, SORT},
        MessageExt,
    },
    BotResult, Context, Error,
};

use std::sync::Arc;
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{application_command::CommandOptionValue, ApplicationCommand},
};

enum OsustatsCommandKind {
    Count(CountArgs),
    Players(OsuStatsListParams),
    Scores(ScoresArgs),
}

impl OsustatsCommandKind {
    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let option = command
            .data
            .options
            .pop()
            .ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "count" => match CountArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(Self::Count(args))),
                    Err(content) => Ok(Err(content)),
                },
                "players" => match OsuStatsListParams::slash(options)? {
                    Ok(args) => Ok(Ok(Self::Players(args))),
                    Err(content) => Ok(Err(content.into())),
                },
                "scores" => match ScoresArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(Self::Scores(args))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }
}

pub async fn slash_osustats(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match OsustatsCommandKind::slash(&ctx, &mut command).await? {
        Ok(OsustatsCommandKind::Count(args)) => _count(ctx, command.into(), args).await,
        Ok(OsustatsCommandKind::Players(args)) => _players(ctx, command.into(), args).await,
        Ok(OsustatsCommandKind::Scores(args)) => _scores(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

fn option_min_rank() -> MyCommandOption {
    MyCommandOption::builder("min_rank", "Specify a min rank between 1 and 100")
        .integer(Vec::new(), false)
}

fn option_max_rank() -> MyCommandOption {
    MyCommandOption::builder("max_rank", "Specify a max rank between 1 and 100")
        .integer(Vec::new(), false)
}

pub fn define_osustats() -> MyCommand {
    let mode = option_mode();
    let name = option_name();
    let discord = option_discord();

    let count_description =
        "Count how often a user appears on top of map leaderboards (same as `/osc`)";

    let count =
        MyCommandOption::builder("count", count_description).subcommand(vec![mode, name, discord]);

    let mode = option_mode();
    let country = option_country();
    let min_rank = option_min_rank();
    let max_rank = option_max_rank();

    let players_description = "National player leaderboard of global leaderboard counts";

    let players = MyCommandOption::builder("players", players_description)
        .help("List players of a country and how often they appear on global map leaderboards.")
        .subcommand(vec![mode, country, min_rank, max_rank]);

    let mode = option_mode();
    let name = option_name();

    let sort_choices = vec![
        CommandOptionChoice::String {
            name: ACCURACY.to_owned(),
            value: ACC.to_owned(),
        },
        CommandOptionChoice::String {
            name: COMBO.to_owned(),
            value: COMBO.to_owned(),
        },
        CommandOptionChoice::String {
            name: MISSES.to_owned(),
            value: MISSES.to_owned(),
        },
        CommandOptionChoice::String {
            name: "pp".to_owned(),
            value: "pp".to_owned(),
        },
        CommandOptionChoice::String {
            name: RANK.to_owned(),
            value: RANK.to_owned(),
        },
        CommandOptionChoice::String {
            name: SCORE.to_owned(),
            value: SCORE.to_owned(),
        },
        CommandOptionChoice::String {
            name: "score date".to_owned(),
            value: "date".to_owned(),
        },
    ];

    let sort_help = "Choose how the scores should be ordered.\n\
        If not specified, it orders them by score date.";

    let sort = MyCommandOption::builder(SORT, "Choose how the scores should be ordered")
        .help(sort_help)
        .string(sort_choices, false);

    let mods = option_mods_explicit();
    let min_rank = option_min_rank();
    let max_rank = option_max_rank();

    // TODO
    // let min_acc =
    //     MyCommandOption::builder("min_acc", "Specify a min accuracy between 0.0 and 100.0")
    //         .number(Vec::new(), false);

    // let max_acc =
    //     MyCommandOption::builder("max_acc", "Specify a max accuracy between 0.0 and 100.0")
    //         .number(Vec::new(), false);

    let min_acc =
        MyCommandOption::builder("min_acc", "Specify a min accuracy between 0.0 and 100.0")
            .string(Vec::new(), false);

    let max_acc =
        MyCommandOption::builder("max_acc", "Specify a max accuracy between 0.0 and 100.0")
            .string(Vec::new(), false);

    let reverse =
        MyCommandOption::builder(REVERSE, "Reverse the resulting score list").boolean(false);

    let discord = option_discord();

    let scores_description = "All scores of a player that are on a map's global leaderboard";

    let scores = MyCommandOption::builder("scores", scores_description).subcommand(vec![
        mode, name, sort, mods, min_rank, max_rank, min_acc, max_acc, reverse, discord,
    ]);

    let description = "Stats about players' appearances in maps' leaderboards";

    let help = "Stats about scores that players have on maps' global leaderboards.\n\
        All data is provided by [osustats](https://osustats.ppy.sh/).\n\
        Note that the data usually __updates once per day__.";

    MyCommand::new("osustats", description)
        .help(help)
        .options(vec![count, players, scores])
}
