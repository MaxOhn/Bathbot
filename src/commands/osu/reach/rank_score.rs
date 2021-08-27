use crate::{
    database::UserConfig,
    embeds::{EmbedData, RankRankedScoreEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    Args, BotResult, CommandData, Context, Error,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::sync::Arc;
use twilight_model::{
    application::interaction::application_command::CommandDataOption, id::UserId,
};

pub(super) async fn _rankscore(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RankScoreArgs,
) -> BotResult<()> {
    let RankScoreArgs { config, rank } = args;

    let name = match config.name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let mode = config.mode.unwrap_or(GameMode::STD);

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
            match RankScoreArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut rank_args)) => {
                    rank_args.config.mode = Some(rank_args.config.mode(GameMode::STD));

                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
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
            match RankScoreArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut rank_args)) => {
                    rank_args.config.mode = Some(GameMode::MNA);

                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
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
            match RankScoreArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut rank_args)) => {
                    rank_args.config.mode = Some(GameMode::TKO);

                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
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
            match RankScoreArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut rank_args)) => {
                    rank_args.config.mode = Some(GameMode::CTB);

                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
    }
}

pub(super) struct RankScoreArgs {
    pub config: UserConfig,
    pub rank: usize,
}

impl RankScoreArgs {
    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: UserId,
    ) -> BotResult<Result<Self, &'static str>> {
        let mut config = ctx.user_config(author_id).await?;
        let mut rank = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => rank = Some(num),
                Err(_) => match Args::check_user_mention(ctx, arg).await? {
                    Ok(name) => config.name = Some(name),
                    Err(content) => return Ok(Err(content)),
                },
            }
        }

        let rank = match rank {
            Some(rank) => rank,
            None => return Ok(Err("You must specify a target rank")),
        };

        Ok(Ok(Self { config, rank }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
        author_id: UserId,
    ) -> BotResult<Result<Self, String>> {
        let mut config = ctx.user_config(author_id).await?;
        let mut rank = None;

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "mode" => config.mode = parse_mode_option!(value, "rank pp"),
                    "name" => config.name = Some(value.into()),
                    "discord" => config.name = parse_discord_option!(ctx, value, "rank pp"),
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

        Ok(Ok(RankScoreArgs { config, rank }))
    }
}
