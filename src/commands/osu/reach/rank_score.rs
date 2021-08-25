use crate::{
    embeds::{EmbedData, RankRankedScoreEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, Name,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::application::interaction::application_command::CommandDataOption;

pub(super) async fn _rankscore(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RankScoreArgs,
) -> BotResult<()> {
    let RankScoreArgs {
        name,
        mut mode,
        rank,
    } = args;

    let author_id = data.author()?.id;

    mode = match ctx.user_config(author_id).await {
        Ok(config) => config.mode(mode),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(author_id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    if rank == 0 {
        let content = "Rank number must be between 1 and 10,000";

        return data.error(&ctx, content).await;
    } else if rank > 10_000 {
        let content = "Unfortunately I can only provide data for ranks up to 10,000 :(";

        return data.error(&ctx, content).await;
    }

    // Retrieve the user and the user thats holding the given rank
    let page = (rank / 50) + (rank % 50 != 0) as usize;
    let rank_holder_fut = ctx.osu().score_rankings(mode).page(page as u32);
    let user_fut = super::request_user(&ctx, &name, Some(mode));

    let (user, rank_holder) = match tokio::try_join!(user_fut, rank_holder_fut) {
        Ok((user, mut rankings)) => {
            let idx = (rank + 49) % 50;
            let rank_holder = rankings.ranking.swap_remove(idx);

            (user, rank_holder)
        }
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let embed_data = RankRankedScoreEmbed::new(user, rank, rank_holder);

    // Creating the embed
    let embed = embed_data.into_builder().build();
    data.create_message(&ctx, embed.into()).await?;

    Ok(())
}

#[command]
#[short_desc("How much ranked score is a player missing to reach the given rank?")]
#[long_desc(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[aliases("rrs")]
pub async fn rankrankedscore(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankScoreArgs::args(&ctx, &mut args, GameMode::STD) {
                Ok(rank_args) => {
                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
    }
}

#[command]
#[short_desc("How much ranked score is a player missing to reach the given rank?")]
#[long_desc(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[aliases("rrsm")]
pub async fn rankrankedscoremania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankScoreArgs::args(&ctx, &mut args, GameMode::MNA) {
                Ok(rank_args) => {
                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
    }
}

#[command]
#[short_desc("How much ranked score is a player missing to reach the given rank?")]
#[long_desc(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[aliases("rrst")]
pub async fn rankrankedscoretaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankScoreArgs::args(&ctx, &mut args, GameMode::TKO) {
                Ok(rank_args) => {
                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
    }
}

#[command]
#[short_desc("How much ranked score is a player missing to reach the given rank?")]
#[long_desc(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[aliases("rrsc")]
pub async fn rankrankedscorectb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankScoreArgs::args(&ctx, &mut args, GameMode::CTB) {
                Ok(rank_args) => {
                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
    }
}

pub(super) struct RankScoreArgs {
    pub name: Option<Name>,
    pub mode: GameMode,
    pub rank: usize,
}

impl RankScoreArgs {
    fn args(ctx: &Context, args: &mut Args<'_>, mode: GameMode) -> Result<Self, &'static str> {
        let mut name = None;
        let mut rank = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => rank = Some(num),
                Err(_) => name = Some(Args::try_link_name(ctx, arg)?),
            }
        }

        let rank = rank.ok_or("You must specify a target rank.")?;

        Ok(Self { name, mode, rank })
    }

    pub(super) fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
    ) -> BotResult<Result<Self, String>> {
        let mut username = None;
        let mut mode = None;
        let mut rank = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "mode" => mode = parse_mode_option!(value, "rank pp"),
                    "name" => username = Some(value.into()),
                    "discord" => match value.parse() {
                        Ok(id) => match ctx.get_link(id) {
                            Some(name) => username = Some(name),
                            None => {
                                let content = format!("<@{}> is not linked to an osu profile", id);

                                return Ok(Err(content.into()));
                            }
                        },
                        Err(_) => {
                            bail_cmd_option!("rank pp discord", string, value)
                        }
                    },
                    _ => bail_cmd_option!("rank pp", string, name),
                },
                CommandDataOption::Integer { name, value } => match name.as_str() {
                    "rank" => rank = Some(value.max(0) as usize),
                    _ => bail_cmd_option!("rank pp", integer, name),
                },
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("rank pp", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("rank pp", subcommand, name)
                }
            }
        }

        let rank = rank.ok_or(Error::InvalidCommandOptions)?;
        let mode = mode.unwrap_or(GameMode::STD);

        let args = RankScoreArgs {
            name: username,
            mode,
            rank,
        };

        Ok(Ok(args))
    }
}
