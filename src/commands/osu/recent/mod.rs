mod leaderboard;
mod list;
mod score;
mod simulate;

pub use leaderboard::*;
pub use list::*;
pub use score::*;
pub use simulate::*;

use std::sync::Arc;

use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{application_command::CommandOptionValue, ApplicationCommand},
};

use crate::{
    commands::{
        osu::{
            option_discord, option_mode, option_mods, option_mods_explicit, option_name, TopArgs,
            TopOrder,
        },
        DoubleResultCow, {MyCommand, MyCommandOption},
    },
    util::{
        constants::common_literals::{
            ACC, COMBO, CONSIDER_GRADE, CTB, FRUITS, GRADE, INDEX, MANIA, MISSES, MODE, OSU,
            REVERSE, SCORE, SPECIFY_MODE, TAIKO,
        },
        MessageExt,
    },
    BotResult, Context, Error,
};

use super::{ErrorType, GradeArg, _top, prepare_score, prepare_scores, request_user, require_link};

enum RecentCommandKind {
    Best(TopArgs),
    Leaderboard(RecentLeaderboardArgs),
    List(RecentListArgs),
    Score(RecentArgs),
    Simulate(RecentSimulateArgs),
}

impl RecentCommandKind {
    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let option = command
            .data
            .options
            .pop()
            .ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                SCORE => match RecentArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(RecentCommandKind::Score(args))),
                    Err(content) => Ok(Err(content)),
                },
                "best" => match TopArgs::slash(ctx, command, options).await? {
                    Ok(mut args) => {
                        args.sort_by = TopOrder::Date;

                        Ok(Ok(RecentCommandKind::Best(args)))
                    }
                    Err(content) => Ok(Err(content)),
                },
                "leaderboard" => match RecentLeaderboardArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(RecentCommandKind::Leaderboard(args))),
                    Err(content) => Ok(Err(content)),
                },
                "list" => match RecentListArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(RecentCommandKind::List(args))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            CommandOptionValue::SubCommandGroup(options) => match option.name.as_str() {
                "simulate" => match RecentSimulateArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(RecentCommandKind::Simulate(args))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }
}

pub async fn slash_recent(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match RecentCommandKind::slash(&ctx, &mut command).await? {
        Ok(RecentCommandKind::Score(args)) => _recent(ctx, command.into(), args).await,
        Ok(RecentCommandKind::Best(args)) => _top(ctx, command.into(), args).await,
        Ok(RecentCommandKind::Leaderboard(args)) => {
            _recentleaderboard(ctx, command.into(), args, false).await
        }
        Ok(RecentCommandKind::List(args)) => _recentlist(ctx, command.into(), args).await,
        Ok(RecentCommandKind::Simulate(args)) => _recentsimulate(ctx, command.into(), args).await,
        Err(msg) => command.error(&ctx, msg).await,
    }
}

fn subcommand_score() -> MyCommandOption {
    let mode = MyCommandOption::builder(MODE, SPECIFY_MODE)
        .string(super::mode_choices(), false)
        .help(
            "Specify a gamemode.\n\
            For mania the combo will be displayed as `[ combo / ratio ]` \
            with ratio being `n320/n300`.",
        );

    let name = option_name();

    let index_help = "By default the very last play will be chosen.\n\
        However, if this index is specified, the play at that index will be displayed instead.\n\
        E.g. `index:1` is the default and `index:2` would show the second most recent play.\n\
        The given index should be between 1 and 100.";

    let index = MyCommandOption::builder(INDEX, "Choose the recent score's index")
        .help(index_help)
        .integer(Vec::new(), false);

    let grade = MyCommandOption::builder(GRADE, CONSIDER_GRADE).string(
        vec![
            CommandOptionChoice::String {
                name: "SS".to_owned(),
                value: "SS".to_owned(),
            },
            CommandOptionChoice::String {
                name: "S".to_owned(),
                value: "S".to_owned(),
            },
            CommandOptionChoice::String {
                name: "A".to_owned(),
                value: "A".to_owned(),
            },
            CommandOptionChoice::String {
                name: "B".to_owned(),
                value: "B".to_owned(),
            },
            CommandOptionChoice::String {
                name: "C".to_owned(),
                value: "C".to_owned(),
            },
            CommandOptionChoice::String {
                name: "D".to_owned(),
                value: "D".to_owned(),
            },
            CommandOptionChoice::String {
                name: "F".to_owned(),
                value: "F".to_owned(),
            },
        ],
        false,
    );

    let passes =
        MyCommandOption::builder("passes", "Specify whether only passes should be considered")
            .boolean(false);

    let discord = option_discord();

    MyCommandOption::builder(SCORE, "Show a user's recent score")
        .subcommand(vec![mode, name, index, grade, passes, discord])
}

fn subcommand_best() -> MyCommandOption {
    let mode = option_mode();
    let name = option_name();
    let mods = option_mods_explicit();

    let index_help =
        "By default the command will show paginated embeds containing five scores per page.\n\
        However, if this index is specified, the command will only show the score at the given index.\n\
        E.g. `index:1` will show the top score and \
        `index:3` will show the score with the third highest pp amount\n\
        The given index should be between 1 and 100.";

    let index = MyCommandOption::builder(INDEX, "Choose a specific score index between 1 and 100")
        .help(index_help)
        .integer(Vec::new(), false);

    let discord = option_discord();
    let reverse =
        MyCommandOption::builder(REVERSE, "Reverse the resulting score list").boolean(false);

    let grade = MyCommandOption::builder(GRADE, CONSIDER_GRADE).string(
        vec![
            CommandOptionChoice::String {
                name: "SS".to_owned(),
                value: "SS".to_owned(),
            },
            CommandOptionChoice::String {
                name: "S".to_owned(),
                value: "S".to_owned(),
            },
            CommandOptionChoice::String {
                name: "A".to_owned(),
                value: "A".to_owned(),
            },
            CommandOptionChoice::String {
                name: "B".to_owned(),
                value: "B".to_owned(),
            },
            CommandOptionChoice::String {
                name: "C".to_owned(),
                value: "C".to_owned(),
            },
            CommandOptionChoice::String {
                name: "D".to_owned(),
                value: "D".to_owned(),
            },
        ],
        false,
    );

    MyCommandOption::builder("best", "Display the user's current top100 sorted by date")
        .subcommand(vec![mode, name, mods, index, discord, reverse, grade])
}

fn subcommand_leaderboard() -> MyCommandOption {
    let mode = option_mode();
    let name = option_name();
    let mods = option_mods(true);

    let index_help = "By default the leaderboard of the very last score will be displayed.\n\
        However, if this index is specified, the leaderboard of the score at that index will be displayed instead.\n\
        E.g. `index:1` is the default and `index:2` for the second most recent play.\n\
        The given index should be between 1 and 100.";

    let index = MyCommandOption::builder(INDEX, "Choose the recent score's index")
        .help(index_help)
        .integer(Vec::new(), false);

    let discord = option_discord();

    let description = "Show the leaderboard of a user's recently played map";

    MyCommandOption::builder("leaderboard", description)
        .subcommand(vec![mode, name, mods, index, discord])
}

fn subcommand_list() -> MyCommandOption {
    let mode = option_mode();
    let name = option_name();

    let grade = MyCommandOption::builder(GRADE, CONSIDER_GRADE).string(
        vec![
            CommandOptionChoice::String {
                name: "SS".to_owned(),
                value: "SS".to_owned(),
            },
            CommandOptionChoice::String {
                name: "S".to_owned(),
                value: "S".to_owned(),
            },
            CommandOptionChoice::String {
                name: "A".to_owned(),
                value: "A".to_owned(),
            },
            CommandOptionChoice::String {
                name: "B".to_owned(),
                value: "B".to_owned(),
            },
            CommandOptionChoice::String {
                name: "C".to_owned(),
                value: "C".to_owned(),
            },
            CommandOptionChoice::String {
                name: "D".to_owned(),
                value: "D".to_owned(),
            },
            CommandOptionChoice::String {
                name: "F".to_owned(),
                value: "F".to_owned(),
            },
        ],
        false,
    );

    let passes =
        MyCommandOption::builder("passes", "Specify whether only passes should be considered")
            .boolean(false);

    let discord = option_discord();

    MyCommandOption::builder("list", "Show all recent plays of a user")
        .subcommand(vec![mode, name, grade, passes, discord])
}

fn subcommand_simulate() -> MyCommandOption {
    fn simulate_index() -> MyCommandOption {
        let help = "By default the very last play will be chosen.\n\
            However, if this index is specified, the play at that index will be chosen instead.\n\
            E.g. `index:1` is the default and `index:2` would take the second most recent play.\n\
            The given index should be between 1 and 100.";

        MyCommandOption::builder(INDEX, "Choose the recent score's index")
            .help(help)
            .integer(Vec::new(), false)
    }

    let name = option_name();
    let mods = option_mods(false);
    let index = simulate_index();

    let n300 =
        MyCommandOption::builder("n300", "Specify the amount of 300s").integer(Vec::new(), false);

    let n100 =
        MyCommandOption::builder("n100", "Specify the amount of 100s").integer(Vec::new(), false);

    let n50 =
        MyCommandOption::builder("n50", "Specify the amount of 50s").integer(Vec::new(), false);

    let misses =
        MyCommandOption::builder(MISSES, "Specify the amount of misses").integer(Vec::new(), false);

    // TODO
    // let acc = MyCommandOption::builder(ACC, "Specify the accuracy")
    //     .help("Specify the accuracy. Should be between 0.0 and 100.0")
    //     .number(Vec::new(), false);

    let acc = MyCommandOption::builder(ACC, "Specify the accuracy")
        .help("Specify the accuracy. Should be between 0.0 and 100.0")
        .string(Vec::new(), false);

    let combo = MyCommandOption::builder(COMBO, "Specify the combo").integer(Vec::new(), false);

    let discord = option_discord();

    let osu_help = "Simulate an osu!standard score.\n\
        If no hitresults, combo, or acc are specified, it will unchoke the score.";

    let osu = MyCommandOption::builder(OSU, "Simulate an osu!standard score")
        .help(osu_help)
        .subcommand(vec![
            name, mods, index, n300, n100, n50, misses, acc, combo, discord,
        ]);

    let name = option_name();
    let mods = option_mods(false);
    let index = simulate_index();

    let n300 =
        MyCommandOption::builder("n300", "Specify the amount of 300s").integer(Vec::new(), false);

    let n100 =
        MyCommandOption::builder("n100", "Specify the amount of 100s").integer(Vec::new(), false);

    let misses =
        MyCommandOption::builder(MISSES, "Specify the amount of misses").integer(Vec::new(), false);

    let acc = MyCommandOption::builder(ACC, "Specify the accuracy")
        .help("Specify the accuracy. Should be between 0.0 and 100.0")
        .string(Vec::new(), false);

    let combo = MyCommandOption::builder(COMBO, "Specify the combo").integer(Vec::new(), false);

    let discord = option_discord();

    let taiko_help = "Simulate an osu!taiko score.\n\
        If no hitresults, combo, or acc are specified, it will unchoke the score.";

    let taiko = MyCommandOption::builder(TAIKO, "Simulate an osu!taiko score")
        .help(taiko_help)
        .subcommand(vec![
            name, mods, index, n300, n100, misses, acc, combo, discord,
        ]);

    let name = option_name();
    let mods = option_mods(false);
    let index = simulate_index();

    let fruits = MyCommandOption::builder(FRUITS, "Specify the amount of fruit hits")
        .integer(Vec::new(), false);

    let droplets = MyCommandOption::builder("droplets", "Specify the amount of droplet hits")
        .integer(Vec::new(), false);

    let tiny_droplets =
        MyCommandOption::builder("tiny_droplets", "Specify the amount of tiny droplet hits")
            .integer(Vec::new(), false);

    let misses = MyCommandOption::builder(MISSES, "Specify the amount of fruit misses")
        .integer(Vec::new(), false);

    let acc = MyCommandOption::builder(ACC, "Specify the accuracy")
        .help("Specify the accuracy. Should be between 0.0 and 100.0")
        .string(Vec::new(), false);

    let combo = MyCommandOption::builder(COMBO, "Specify the combo").integer(Vec::new(), false);

    let discord = option_discord();

    let catch_help = "Simulate an osu!ctb score.\n\
        If no hitresults, combo, or acc are specified, it will unchoke the score.";

    let catch_options = vec![
        name,
        mods,
        index,
        fruits,
        droplets,
        tiny_droplets,
        misses,
        acc,
        combo,
        discord,
    ];

    let catch = MyCommandOption::builder(CTB, "Simulate an osu!ctb score")
        .help(catch_help)
        .subcommand(catch_options);

    let name = option_name();
    let mods = option_mods(false);
    let index = simulate_index();

    let score_help = "Mania calculations don't depend on specific hitresults, accuracy or combo.\n\
        Instead it just requires the score.\n\
        The value should be between 0 and 1,000,000 and already adjusted to mods \
        e.g. only up to 500,000 for `EZ` or up to 250,000 for `EZNF`.";

    let score = MyCommandOption::builder(SCORE, "Specify the score")
        .help(score_help)
        .integer(Vec::new(), false);

    let discord = option_discord();

    let mania_help = "Simulate an osu!mania score.\n\
        If `score` is not specified, a perfect play will be shown.";

    let mania = MyCommandOption::builder(MANIA, "Simulate an osu!mania score")
        .help(mania_help)
        .subcommand(vec![name, mods, index, score, discord]);

    let description = "Unchoke a user's recent score or simulate a perfect play on its map";

    MyCommandOption::builder("simulate", description)
        .subcommandgroup(vec![osu, taiko, catch, mania])
}

pub fn define_recent() -> MyCommand {
    let help = "Retrieve a user's recent plays and display them in various forms.\n\
        The osu!api can provide the last 100 recent plays done within the last 24 hours.";

    let options = vec![
        subcommand_score(),
        subcommand_best(),
        subcommand_leaderboard(),
        subcommand_list(),
        subcommand_simulate(),
    ];

    MyCommand::new("recent", "Display info about a user's recent plays")
        .help(help)
        .options(options)
}
