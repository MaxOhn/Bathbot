mod pp;
mod rank_pp;
mod rank_score;

pub use pp::*;
pub use rank_pp::*;
pub use rank_score::*;

use std::{borrow::Cow, sync::Arc};

use twilight_model::application::interaction::{
    application_command::CommandDataOption, ApplicationCommand,
};

use crate::{BotResult, Context, Error, commands::{
        osu::{option_country, option_discord, option_mode, option_name},
        MyCommand, MyCommandOption,
    }, util::{ApplicationCommandExt, InteractionExt, MessageExt, constants::common_literals::{RANK, SCORE}}};

use super::{request_user, require_link};

enum ReachCommandKind {
    Performance(PpArgs),
    RankPerformance(RankPpArgs),
    RankScore(RankScoreArgs),
}

const REACH_RANK: &str = "reach rank";

impl ReachCommandKind {
    async fn slash(
        ctx: &Context,
        command: &mut ApplicationCommand,
    ) -> BotResult<Result<Self, Cow<'static, str>>> {
        let author_id = command.user_id()?;
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => bail_cmd_option!("reach", string, name),
                CommandDataOption::Integer { name, .. } => bail_cmd_option!("reach", integer, name),
                CommandDataOption::Boolean { name, .. } => bail_cmd_option!("reach", boolean, name),
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "pp" => match PpArgs::slash(ctx, options, author_id).await? {
                        Ok(args) => kind = Some(Self::Performance(args)),
                        Err(content) => return Ok(Err(content)),
                    },
                    RANK => {
                        for option in options {
                            match option {
                                CommandDataOption::String { name, .. } => {
                                    bail_cmd_option!(REACH_RANK, string, name)
                                }
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!(REACH_RANK, integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!(REACH_RANK, boolean, name)
                                }
                                CommandDataOption::SubCommand { name, options } => {
                                    match name.as_str() {
                                        "pp" => match RankPpArgs::slash(ctx, options, author_id)
                                            .await?
                                        {
                                            Ok(args) => kind = Some(Self::RankPerformance(args)),
                                            Err(content) => return Ok(Err(content.into())),
                                        },
                                        SCORE => {
                                            match RankScoreArgs::slash(ctx, options, author_id)
                                                .await?
                                            {
                                                Ok(args) => kind = Some(Self::RankScore(args)),
                                                Err(content) => return Ok(Err(content.into())),
                                            }
                                        }
                                        _ => bail_cmd_option!(REACH_RANK, subcommand, name),
                                    }
                                }
                            }
                        }
                    }
                    _ => bail_cmd_option!("reach", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions).map(Ok)
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

    // TODO: Number variant
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
