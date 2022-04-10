use std::sync::Arc;

use command_macros::command;
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::osu::{get_user, UserArgs},
    database::UserConfig,
    embeds::{EmbedData, RankRankedScoreEmbed},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        InteractionExt, MessageExt,
    },
    BotResult, Context, Error,
};

pub(super) async fn score(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RankScore<'_>,
) -> BotResult<()> {
    let (name, mode) = name_mode!(ctx, orig, args);
    let rank = args.rank;

    if rank == 0 {
        let content = "Rank number must be between 1 and 10,000";

        return orig.error(&ctx, content).await;
    } else if rank > 10_000 {
        let content = "Unfortunately I can only provide data for ranks up to 10,000 :(";

        return orig.error(&ctx, content).await;
    }

    // Retrieve the user and the user thats holding the given rank
    let page = (rank / 50) + (rank % 50 != 0) as usize;
    let rank_holder_fut = ctx.osu().score_rankings(mode).page(page as u32);
    let user_args = UserArgs::new(name.as_str(), mode);
    let user_fut = get_user(&ctx, &user_args);

    let (mut user, rank_holder) = match tokio::try_join!(user_fut, rank_holder_fut) {
        Ok((user, mut rankings)) => {
            let idx = (rank + 49) % 50;
            let rank_holder = rankings.ranking.swap_remove(idx);

            (user, rank_holder)
        }
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Accumulate all necessary data
    let embed_data = RankRankedScoreEmbed::new(user, rank, rank_holder);

    // Creating the embed
    let embed = embed_data.into_builder().build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}

#[command]
#[desc("How much ranked score is a player missing to reach the given rank?")]
#[help(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[alias("rrs")]
#[group(Osu)]
async fn prefix_rankrankedscore(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankScoreArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut rank_args)) => {
                    rank_args.config.mode.get_or_insert(GameMode::STD);

                    _rankscore(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => super::slash_rank(ctx, *command).await,
    }
}

#[command]
#[desc("How much ranked score is a player missing to reach the given rank?")]
#[help(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[alias("rrsm")]
#[group(Mania)]
async fn prefix_rankrankedscoremania(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
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
        CommandData::Interaction { command } => super::slash_rank(ctx, *command).await,
    }
}

#[command]
#[desc("How much ranked score is a player missing to reach the given rank?")]
#[help(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[alias("rrst")]
#[group(Taiko)]
async fn prefix_rankrankedscoretaiko(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
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
        CommandData::Interaction { command } => super::slash_rank(ctx, *command).await,
    }
}

#[command]
#[desc("How much ranked score is a player missing to reach the given rank?")]
#[help(
    "How much score is a player missing to reach the given rank in the ranked score leaderboard?\n\
    The number for the rank must be between 1 and 10,000."
)]
#[usage("[username] [number]")]
#[example("badewanne3 123")]
#[alias("rrsc")]
#[group(Catch)]
async fn prefix_rankrankedscorectb(
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
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
        CommandData::Interaction { command } => super::slash_rank(ctx, *command).await,
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
        author_id: Id<UserMarker>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
        let mut rank = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => rank = Some(num),
                Err(_) => match check_user_mention(ctx, arg).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
                },
            }
        }

        let rank = match rank {
            Some(rank) => rank,
            None => return Ok(Err("You must specify a target rank".into())),
        };

        Ok(Ok(Self { config, rank }))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut rank = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    MODE => config.mode = parse_mode_option(&value),
                    NAME => config.osu = Some(value.into()),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Integer(value) => {
                    let number = (option.name == RANK)
                        .then(|| value)
                        .ok_or(Error::InvalidCommandOptions)?;

                    rank = Some(number.max(0) as usize);
                }
                CommandOptionValue::User(value) => match option.name.as_str() {
                    DISCORD => match parse_discord(ctx, value).await? {
                        Ok(osu) => config.osu = Some(osu),
                        Err(content) => return Ok(Err(content)),
                    },
                    _ => return Err(Error::InvalidCommandOptions),
                },
                _ => return Err(Error::InvalidCommandOptions),
            }
        }

        let rank = rank.ok_or(Error::InvalidCommandOptions)?;

        Ok(Ok(RankScoreArgs { config, rank }))
    }
}
