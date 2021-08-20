use crate::{
    custom_client::RankParam,
    embeds::{EmbedData, RankEmbed},
    tracking::process_tracking,
    util::{
        constants::{OSU_API_ISSUE, OSU_DAILY_ISSUE},
        CountryCode, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, Name,
};

use rosu_v2::prelude::{GameMode, OsuError, User, UserCompact};
use std::sync::Arc;
use twilight_model::application::interaction::application_command::CommandDataOption;

pub(super) async fn _rank(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RankPpArgs,
) -> BotResult<()> {
    let RankPpArgs {
        name,
        mode,
        country,
        rank,
    } = args;

    let name = match name {
        Some(name) => name,
        None => match ctx.get_link(data.author()?.id.0) {
            Some(name) => name,
            None => return super::require_link(&ctx, &data).await,
        },
    };

    if rank == 0 {
        return data.error(&ctx, "Rank can't be zero :clown:").await;
    } else if rank > 10_000 && country.is_some() {
        let content = "Unfortunately I can only provide data for country ranks up to 10,000 :(";

        return data.error(&ctx, content).await;
    }

    let rank_data = if rank <= 10_000 {
        // Retrieve the user and the user thats holding the given rank
        let page = (rank / 50) + (rank % 50 != 0) as usize;

        let mut rank_holder_fut = ctx.osu().performance_rankings(mode).page(page as u32);

        if let Some(ref country) = country {
            rank_holder_fut = rank_holder_fut.country(country.as_str());
        }

        let user_fut = super::request_user(&ctx, &name, Some(mode));

        let (user, rank_holder) = match tokio::try_join!(user_fut, rank_holder_fut) {
            Ok((user, mut rankings)) => {
                let idx = (args.rank + 49) % 50;
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

        RankData::Sub10k {
            user,
            rank,
            country,
            rank_holder,
        }
    } else {
        let pp_fut = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Rank(rank));

        let user_fut = super::request_user(&ctx, &name, Some(mode));
        let (pp_result, user_result) = tokio::join!(pp_fut, user_fut);

        let required_pp = match pp_result {
            Ok(rank_pp) => rank_pp.pp,
            Err(why) => {
                let _ = data.error(&ctx, OSU_DAILY_ISSUE).await;

                return Err(why.into());
            }
        };

        let user = match user_result {
            Ok(user) => user,
            Err(OsuError::NotFound) => {
                let content = format!("User `{}` was not found", name);

                return data.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        };

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
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }
    } else {
        None
    };

    if let Some(ref mut scores) = scores {
        // Process user and their top scores for tracking
        process_tracking(&ctx, mode, scores, Some(rank_data.user_borrow())).await;
    }

    // Creating the embed
    let embed = RankEmbed::new(rank_data, scores).into_builder().build();
    data.create_message(&ctx, embed.into()).await?;

    Ok(())
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("reach")]
pub async fn rank(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankPpArgs::args(&ctx, &mut args, GameMode::STD) {
                Ok(rank_args) => {
                    _rank(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
    }
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("rankm", "reachmania", "reachm")]
pub async fn rankmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankPpArgs::args(&ctx, &mut args, GameMode::MNA) {
                Ok(rank_args) => {
                    _rank(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
    }
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("rankt", "reachtaiko", "reacht")]
pub async fn ranktaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankPpArgs::args(&ctx, &mut args, GameMode::TKO) {
                Ok(rank_args) => {
                    _rank(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
    }
}

#[command]
#[short_desc("How many pp is a player missing to reach the given rank?")]
#[long_desc(
    "How many pp is a player missing to reach the given rank?\n\
    For ranks over 10,000 the data is provided by [osudaily](https://osudaily.net/)."
)]
#[usage("[username] [[country]number]")]
#[example("badewanne3 be50", "badewanne3 123")]
#[aliases("rankc", "reachctb", "reachc")]
pub async fn rankctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match RankPpArgs::args(&ctx, &mut args, GameMode::CTB) {
                Ok(rank_args) => {
                    _rank(ctx, CommandData::Message { msg, args, num }, rank_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => super::slash_reach(ctx, command).await,
    }
}

pub enum RankData {
    Sub10k {
        user: User,
        rank: usize,
        country: Option<CountryCode>,
        rank_holder: UserCompact,
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
    pub name: Option<Name>,
    pub mode: GameMode,
    pub country: Option<CountryCode>,
    pub rank: usize,
}

impl RankPpArgs {
    fn args(ctx: &Context, args: &mut Args<'_>, mode: GameMode) -> Result<Self, &'static str> {
        let mut name = None;
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
                            name = Some(Args::try_link_name(ctx, arg)?);
                        }
                    } else {
                        name = Some(Args::try_link_name(ctx, arg)?);
                    }
                }
            }
        }

        const ERR_PARSE_COUNTRY: &str =
            "Failed to parse `rank`. Provide it either as positive number \
            or as country acronym followed by a positive number e.g. `be10` \
            as one of the first two arguments.";

        let rank = rank.ok_or(ERR_PARSE_COUNTRY)?;

        Ok(Self {
            name,
            mode,
            country,
            rank,
        })
    }

    pub(super) fn slash(
        ctx: &Context,
        options: Vec<CommandDataOption>,
    ) -> BotResult<Result<Self, String>> {
        let mut username = None;
        let mut mode = None;
        let mut country = None;
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
                    "country" => {
                        if value.len() == 2 && value.is_ascii() {
                            country = Some(value.into())
                        } else if let Some(code) = CountryCode::from_name(value.as_str()) {
                            country = Some(code);
                        } else {
                            let content = format!(
                                "Failed to parse `{}` as country.\n\
                                Be sure to specify a valid country or two ASCII letter country code.",
                                value
                            );

                            return Ok(Err(content));
                        }
                    }
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

        let args = RankPpArgs {
            name: username,
            country,
            mode,
            rank,
        };

        Ok(Ok(args))
    }
}
