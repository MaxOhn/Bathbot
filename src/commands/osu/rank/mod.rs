mod pp;
mod score;

use std::sync::Arc;

use twilight_model::application::interaction::{
    application_command::CommandOptionValue, ApplicationCommand,
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

pub use self::{pp::*, score::*};

use super::require_link;

enum RankCommandKind {
    Performance(RankPpArgs),
    Score(RankScoreArgs),
}

impl RankCommandKind {
    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let option = command
            .data
            .options
            .pop()
            .ok_or(Error::InvalidCommandOptions)?;

        match option.value {
            CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                "pp" => match RankPpArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(Self::Performance(args))),
                    Err(content) => Ok(Err(content)),
                },
                SCORE => match RankScoreArgs::slash(ctx, command, options).await? {
                    Ok(args) => Ok(Ok(Self::Score(args))),
                    Err(content) => Ok(Err(content)),
                },
                _ => Err(Error::InvalidCommandOptions),
            },
            _ => Err(Error::InvalidCommandOptions),
        }
    }
}

pub async fn slash_rank(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match RankCommandKind::slash(&ctx, &mut command).await? {
        Ok(RankCommandKind::Performance(args)) => _rank(ctx, command.into(), args).await,
        Ok(RankCommandKind::Score(args)) => _rankscore(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn define_rank() -> MyCommand {
    let rank = MyCommandOption::builder(RANK, "Specify the target rank")
        .min_int(1)
        .integer(Vec::new(), true);

    let mode = option_mode();
    let name = option_name();
    let country = option_country();
    let discord = option_discord();

    let each_description =
        "Fill a top100 with scores of this many pp until the pp of the target rank are reached";

    let each = MyCommandOption::builder("each", each_description)
        .min_num(0.0)
        .number(Vec::new(), false);

    let pp = MyCommandOption::builder("pp", "How many pp are missing to reach the given rank?")
        .subcommand(vec![rank, mode, name, each, country, discord]);

    let rank = MyCommandOption::builder(RANK, "Specify the target rank")
        .min_int(1)
        .integer(Vec::new(), true);

    let mode = option_mode();
    let name = option_name();
    let discord = option_discord();

    let score_description = "How much ranked score is missing to reach the given rank?";

    let score = MyCommandOption::builder(SCORE, score_description)
        .subcommand(vec![rank, mode, name, discord]);

    MyCommand::new(RANK, "How much is missing to reach the given rank?").options(vec![pp, score])
}
