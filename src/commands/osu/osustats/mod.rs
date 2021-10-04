mod counts;
mod globals;
mod list;

pub use counts::*;
pub use globals::*;
pub use list::*;

use super::{get_globals_count, request_user, require_link};

use crate::{
    commands::{
        osu::{option_country, option_discord, option_mode, option_mods_explicit, option_name},
        MyCommand, MyCommandOption,
    },
    custom_client::OsuStatsListParams,
    util::{
        constants::common_literals::{ACC, ACCURACY, COMBO, MISSES, RANK, REVERSE, SCORE, SORT},
        ApplicationCommandExt, InteractionExt, MessageExt,
    },
    BotResult, Context, Error,
};

use std::{borrow::Cow, sync::Arc};
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

enum OsustatsCommandKind {
    Count(CountArgs),
    Players(OsuStatsListParams),
    Scores(ScoresArgs),
}

const OSUSTATS: &str = "osustats";

impl OsustatsCommandKind {
    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let author_id = command.user_id()?;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!(OSUSTATS, string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!(OSUSTATS, integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!(OSUSTATS, boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "count" => match CountArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(Self::Count(args)),
                        Err(content) => return Ok(Err(content.into())),
                    },
                    "players" => match OsuStatsListParams::slash(options)? {
                        Ok(args) => kind = Some(Self::Players(args)),
                        Err(content) => return Ok(Err(content.into())),
                    },
                    "scores" => match ScoresArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(Self::Scores(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => bail_cmd_option!(OSUSTATS, subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
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

    let count_description = "Count how often a user appears on top of maps' leaderboards";

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

    // TODO: Number variant
    let min_acc =
        MyCommandOption::builder("min_acc", "Specify a min accuracy between 0.0 and 100.0")
            .string(Vec::new(), false);

    // TODO: Number variant
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

    MyCommand::new(OSUSTATS, description)
        .help(help)
        .options(vec![count, players, scores])
}
