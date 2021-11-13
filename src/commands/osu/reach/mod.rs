mod pp;
mod rank_pp;
mod rank_score;

pub use pp::*;
pub use rank_pp::*;
pub use rank_score::*;

use std::sync::Arc;

use twilight_model::application::interaction::{
    application_command::{CommandDataOption, CommandOptionValue},
    ApplicationCommand,
};

use crate::{
    commands::{
        osu::{option_country, option_discord, option_mode, option_name},
        DoubleResultCow, MyCommand, MyCommandOption,
    },
    util::{
        constants::common_literals::{RANK, SCORE},
        MessageExt,
    },
    BotResult, Context, Error,
};

use super::{request_user, require_link};

enum ReachCommandKind {
    Performance(PpArgs),
    RankPerformance(RankPpArgs),
    RankScore(RankScoreArgs),
}

impl ReachCommandKind {
    async fn slash_rank(
        ctx: &Context,
        command: &ApplicationCommand,
        mut options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let option = options.pop().ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "pp" => match RankPpArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(Self::RankPerformance(args))),
                    Err(content) => Ok(Err(content)),
                },
                SCORE => match RankScoreArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(Self::RankScore(args))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }

    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let option = command
            .data
            .options
            .pop()
            .ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "pp" => match PpArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(Self::Performance(args))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            CommandOptionValue::SubCommandGroup(options) => match option.name.as_str() {
                RANK => Self::slash_rank(ctx, command, options).await,
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }
}

pub async fn slash_reach(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match ReachCommandKind::slash(&ctx, &mut command).await? {
        Ok(ReachCommandKind::Performance(args)) => _pp(ctx, command.into(), args).await,
        Ok(ReachCommandKind::RankPerformance(args)) => _rank(ctx, command.into(), args).await,
        Ok(ReachCommandKind::RankScore(args)) => _rankscore(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn define_reach() -> MyCommand {
    let rank = MyCommandOption::builder(RANK, "Specify the target rank").integer(Vec::new(), true);
    let mode = option_mode();
    let name = option_name();
    let country = option_country();
    let discord = option_discord();

    let pp = MyCommandOption::builder("pp", "How many pp are missing to reach the given rank?")
        .subcommand(vec![rank, mode, name, country, discord]);

    let rank = MyCommandOption::builder(RANK, "Specify the target rank").integer(Vec::new(), true);
    let mode = option_mode();
    let name = option_name();
    let discord = option_discord();

    let score_description = "How much ranked score is missing to reach the given rank?";

    let score = MyCommandOption::builder(SCORE, score_description)
        .subcommand(vec![rank, mode, name, discord]);

    let rank = MyCommandOption::builder(RANK, "How much is missing to reach the given rank?")
        .subcommandgroup(vec![pp, score]);

    // TODO
    // let pp = MyCommandOption::builder("pp", "Specify a target pp amount").number(Vec::new(), true);
    let pp = MyCommandOption::builder("pp", "Specify a target pp amount").string(Vec::new(), true);
    let mode = option_mode();
    let name = option_name();
    let discord = option_discord();
    let pp_description = "How many pp is a user missing to reach the given amount?";

    let pp =
        MyCommandOption::builder("pp", pp_description).subcommand(vec![pp, mode, name, discord]);

    let description = "How much is a user missing to reach the given pp or rank?";

    MyCommand::new("reach", description).options(vec![pp, rank])
}
