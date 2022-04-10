use std::sync::Arc;

use command_macros::command;
use rosu_v2::prelude::{GameMode, OsuError, User, UserCompact};
use twilight_model::{
    application::{
        interaction::{
            ApplicationCommand,
        },
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        osu::{get_user, UserArgs},
    },
    custom_client::RankParam,
    database::UserConfig,
    embeds::{EmbedData, RankEmbed},
    tracking::process_osu_tracking,
    util::{
        constants::{
            GENERAL_ISSUE, OSU_API_ISSUE, OSU_DAILY_ISSUE,
        },
        CountryCode,
    },
    , BotResult,  Context, Error,
};

pub(super) async fn rank(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RankPp<'_>,
) -> BotResult<()> {
    let (name, mode) = name_mode!(ctx, orig, args);

    let RankPp {
        country,
        rank,
        each,
        ..
    } = args;

    if rank == 0 {
        return data.error(&ctx, "Rank can't be zero :clown:").await;
    } else if rank > 10_000 && country.is_some() {
        let content = "Unfortunately I can only provide data for country ranks up to 10,000 :(";

        return orig.error(&ctx, content).await;
    }

    let rank_data = if rank <= 10_000 {
        // Retrieve the user and the user thats holding the given rank
        let page = (rank / 50) + (rank % 50 != 0) as usize;

        let mut rank_holder_fut = ctx.osu().performance_rankings(mode).page(page as u32);

        if let Some(ref country) = country {
            rank_holder_fut = rank_holder_fut.country(country.as_str());
        }

        let user_args = UserArgs::new(name.as_str(), mode);
        let user_fut = get_user(&ctx, &user_args);

        let (mut user, rank_holder) = match tokio::try_join!(user_fut, rank_holder_fut) {
            Ok((user, mut rankings)) => {
                let idx = (args.rank + 49) % 50;
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

        RankData::Sub10k {
            user,
            rank,
            country,
            rank_holder: Box::new(rank_holder),
        }
    } else {
        let pp_fut = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Rank(rank));

        let user_args = UserArgs::new(name.as_str(), mode);
        let user_fut = get_user(&ctx, &user_args);
        let (pp_result, user_result) = tokio::join!(pp_fut, user_fut);

        let required_pp = match pp_result {
            Ok(rank_pp) => rank_pp.pp,
            Err(err) => {
                let _ = orig.error(&ctx, OSU_DAILY_ISSUE).await;

                return Err(err.into());
            }
        };

        let mut user = match user_result {
            Ok(user) => user,
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

        RankData::Over10k {
            user,
            rank: args.rank,
            required_pp,
        }
    };

    // Retrieve the user's top scores if required
    let mut scores = if rank_data.with_scores() {
        let user = rank_data.user_borrow();

        let scores_fut = ctx
            .osu()
            .user_scores(user.user_id)
            .limit(100)
            .best()
            .mode(mode);

        match scores_fut.await {
            Ok(scores) => (!scores.is_empty()).then(|| scores),
            Err(err) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;

                return Err(err.into());
            }
        }
    } else {
        None
    };

    if let Some(ref mut scores) = scores {
        // Process user and their top scores for tracking
        process_osu_tracking(&ctx, scores, Some(rank_data.user_borrow())).await;
    }

    // Creating the embed
    let embed = RankEmbed::new(rank_data, scores, each)
        .into_builder();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}

#[command]
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("reach")]
#[group(Osu)]
async fn prefix_rank(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankPpArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut rank_args)) => {
                    rank_args.config.mode.get_or_insert(GameMode::STD);

                    _rank(ctx, CommandData::Message { msg, args, num }, rank_args).await
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
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("rankm", "reachmania", "reachm")]
#[group(Mania)]
async fn prefix_rankmania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankPpArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut rank_args)) => {
                    rank_args.config.mode = Some(GameMode::MNA);

                    _rank(ctx, CommandData::Message { msg, args, num }, rank_args).await
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
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("rankt", "reachtaiko", "reacht")]
#[group(Taiko)]
async fn prefix_ranktaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankPpArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut rank_args)) => {
                    rank_args.config.mode = Some(GameMode::TKO);

                    _rank(ctx, CommandData::Message { msg, args, num }, rank_args).await
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
#[desc("How many pp is a player missing to reach the given rank?")]
#[help(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[examples("badewanne3 be50", "badewanne3 123")]
#[alias("rankc", "reachctb", "reachc")]
#[group(Catch)]
async fn prefix_rankctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankPpArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(mut rank_args)) => {
                    rank_args.config.mode = Some(GameMode::CTB);

                    _rank(ctx, CommandData::Message { msg, args, num }, rank_args).await
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

pub enum RankData {
    Sub10k {
        user: User,
        rank: usize,
        country: Option<CountryCode>,
        rank_holder: Box<UserCompact>,
    },
    Over10k {
        user: User,
        rank: usize,
        required_pp: f32,
    },
}

impl RankData {
    fn with_scores(&self) -> bool {
        match self {
            Self::Sub10k {
                user, rank_holder, ..
            } => user.statistics.as_ref().unwrap().pp < rank_holder.statistics.as_ref().unwrap().pp,
            Self::Over10k {
                user, required_pp, ..
            } => user.statistics.as_ref().unwrap().pp < *required_pp,
        }
    }

    pub fn user_borrow(&self) -> &User {
        match self {
            Self::Sub10k { user, .. } => user,
            Self::Over10k { user, .. } => user,
        }
    }

    pub fn user(self) -> User {
        match self {
            Self::Sub10k { user, .. } => user,
            Self::Over10k { user, .. } => user,
        }
    }
}

pub(super) struct RankPpArgs {
    pub config: UserConfig,
    pub country: Option<CountryCode>,
    pub rank: usize,
    pub each: Option<f32>,
}

impl RankPpArgs {
    const ERR_PARSE_COUNTRY: &'static str =
        "Failed to parse `rank`. Provide it either as positive number \
        or as country acronym followed by a positive number e.g. `be10` \
        as one of the first two arguments.";

    async fn args(
        ctx: &Context,
        args: &mut Args<'_>,
        author_id: Id<UserMarker>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
        let mut country = None;
        let mut rank = None;

        for arg in args.take(2) {
            match arg.parse() {
                Ok(num) => rank = Some(num),
                Err(_) => {
                    if arg.len() >= 3 {
                        let (prefix, suffix) = arg.split_at(2);
                        let valid_country = prefix.chars().all(|c| c.is_ascii_alphabetic());

                        if let (true, Ok(num)) = (valid_country, suffix.parse()) {
                            country = Some(prefix.to_ascii_uppercase().into());
                            rank = Some(num);
                        } else {
                            match check_user_mention(ctx, arg).await? {
                                Ok(osu) => config.osu = Some(osu),
                                Err(content) => return Ok(Err(content)),
                            }
                        }
                    } else {
                        match check_user_mention(ctx, arg).await? {
                            Ok(osu) => config.osu = Some(osu),
                            Err(content) => return Ok(Err(content)),
                        }
                    }
                }
            }
        }

        let rank = match rank {
            Some(rank) => rank,
            None => return Ok(Err(Self::ERR_PARSE_COUNTRY.into())),
        };

        let args = Self {
            config,
            country,
            rank,
            each: None,
        };

        Ok(Ok(args))
    }

    pub(super) async fn slash(
        ctx: &Context,
        command: &ApplicationCommand,
        options: Vec<CommandDataOption>,
    ) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut country = None;
        let mut rank = None;
        let mut each = None;

        for option in options {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    MODE => config.mode = parse_mode_option(&value),
                    NAME => config.osu = Some(value.into()),
                    COUNTRY => {
                        if value.len() == 2 && value.is_ascii() {
                            country = Some(value.into())
                        } else if let Some(code) = CountryCode::from_name(value.as_str()) {
                            country = Some(code);
                        } else {
                            let content = format!(
                                "Failed to parse `{value}` as country.\n\
                                Be sure to specify a valid country or two ASCII letter country code."
                            );

                            return Ok(Err(content.into()));
                        }
                    }
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Number(Number(value)) => match option.name.as_str() {
                    "each" => each = Some(value as f32),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Integer(value) => match option.name.as_str() {
                    RANK => rank = Some(value as usize),
                    _ => return Err(Error::InvalidCommandOptions),
                },
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

        let args = RankPpArgs {
            rank: rank.ok_or(Error::InvalidCommandOptions)?,
            config,
            country,
            each,
        };

        Ok(Ok(args))
    }
}
