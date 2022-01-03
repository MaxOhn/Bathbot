use std::{mem, sync::Arc};

use rosu_v2::prelude::{GameMode, OsuError};
use twilight_model::{
    application::interaction::{application_command::CommandOptionValue, ApplicationCommand},
    id::UserId,
};

use crate::{
    commands::{
        check_user_mention,
        osu::{option_discord, option_name},
        parse_discord, DoubleResultCow, MyCommand, MyCommandOption,
    },
    database::UserConfig,
    embeds::{BWSEmbed, EmbedData},
    error::Error,
    util::{
        constants::{
            common_literals::{DISCORD, NAME, RANK},
            GENERAL_ISSUE, OSU_API_ISSUE,
        },
        matcher, ApplicationCommandExt, InteractionExt, MessageExt,
    },
    Args, BotResult, CommandData, Context,
};

use super::{get_user, UserArgs};

const MIN_BADGES_OFFSET: usize = 2;

#[command]
#[short_desc("Show the badge weighted seeding for a player")]
#[long_desc(
    "Show the badge weighted seeding for a player. \n\
    The current formula is `rank^(0.9937^(badges^2))`.\n\
    Next to the player's username, you can specify `rank=integer` \
    to show how the bws value progresses towards that rank.\n\
    Similarly, you can specify `badges=integer` to show how the value \
    progresses towards that badge amount."
)]
#[usage("[username] [rank=integer] [badges=integer]")]
#[example("badewanne3", "badewanne3 rank=1234 badges=10", "badewanne3 badges=3")]
async fn bws(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match BwsArgs::args(&ctx, &mut args, msg.author.id).await {
                Ok(Ok(bws_args)) => {
                    _bws(ctx, CommandData::Message { msg, args, num }, bws_args).await
                }
                Ok(Err(content)) => msg.error(&ctx, content).await,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    Err(why)
                }
            }
        }
        CommandData::Interaction { command } => slash_bws(ctx, *command).await,
    }
}

async fn _bws(ctx: Arc<Context>, data: CommandData<'_>, args: BwsArgs) -> BotResult<()> {
    let BwsArgs {
        config,
        rank,
        badges,
    } = args;

    let mode = config.mode.unwrap_or(GameMode::STD);

    let name = match config.into_username() {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_args = UserArgs::new(name.as_str(), mode);

    let user = match get_user(&ctx, &user_args).await {
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

    let badges_curr = user.badges.as_ref().map_or(0, |badges| {
        badges
            .iter()
            .filter(|badge| matcher::tourney_badge(badge.description.as_str()))
            .count()
    });

    let (badges_min, badges_max) = match badges {
        Some(num) => {
            let mut min = num;
            let mut max = badges_curr;

            if min > max {
                mem::swap(&mut min, &mut max);
            }

            max += MIN_BADGES_OFFSET.saturating_sub(max - min);

            (min, max)
        }
        None => (badges_curr, badges_curr + MIN_BADGES_OFFSET),
    };

    let embed_data = BWSEmbed::new(user, badges_curr, badges_min, badges_max, rank);
    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}

struct BwsArgs {
    config: UserConfig,
    rank: Option<u32>,
    badges: Option<usize>,
}

impl BwsArgs {
    async fn args(ctx: &Context, args: &mut Args<'_>, author_id: UserId) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(author_id).await?;
        let mut rank = None;
        let mut badges = None;

        for arg in args.take(3) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    RANK | "r" => match value.parse::<u32>() {
                        Ok(num) => rank = Some(num.max(1)),
                        Err(_) => {
                            let content = "Failed to parse `rank`. Must be a positive integer.";

                            return Ok(Err(content.into()));
                        }
                    },
                    "badges" | "badge" | "b" => match value.parse() {
                        Ok(num) => badges = Some(num),
                        Err(_) => {
                            let content = "Failed to parse `badges`. Must be a positive integer.";

                            return Ok(Err(content.into()));
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `rank` or `badges`.",
                            key
                        );

                        return Ok(Err(content.into()));
                    }
                }
            } else {
                match check_user_mention(ctx, arg).await? {
                    Ok(osu) => config.osu = Some(osu),
                    Err(content) => return Ok(Err(content)),
                }
            }
        }

        let args = Self {
            config,
            rank,
            badges,
        };

        Ok(Ok(args))
    }

    async fn slash(ctx: &Context, command: &mut ApplicationCommand) -> DoubleResultCow<Self> {
        let mut config = ctx.user_config(command.user_id()?).await?;
        let mut rank = None;
        let mut badges = None;

        for option in command.yoink_options() {
            match option.value {
                CommandOptionValue::String(value) => match option.name.as_str() {
                    NAME => config.osu = Some(value.into()),
                    _ => return Err(Error::InvalidCommandOptions),
                },
                CommandOptionValue::Integer(value) => match option.name.as_str() {
                    RANK => rank = Some(value.max(1) as u32),
                    "badges" => badges = Some(value.max(0) as usize),
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

        let args = Self {
            config,
            rank,
            badges,
        };

        Ok(Ok(args))
    }
}

pub async fn slash_bws(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match BwsArgs::slash(&ctx, &mut command).await? {
        Ok(args) => _bws(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn define_bws() -> MyCommand {
    let name = option_name();
    let discord = option_discord();

    let rank_help =
        "If specified, it will calculate how the bws value would evolve towards the given rank.";

    let rank = MyCommandOption::builder(RANK, "Specify a target rank to reach")
        .help(rank_help)
        .integer(Vec::new(), false);

    let badges_help = "Calculate how the bws value evolves towards the given amount of badges.\n\
        If none is specified, it defaults to the current amount + 2.";

    let badges = MyCommandOption::builder("badges", "Specify an amount of badges to reach")
        .help(badges_help)
        .integer(Vec::new(), false);

    let description = "Show the badge weighted seeding for an osu!standard player";

    let help = "To combat those pesky derank players ruining everyone's tourneys, \
        many tournaments use a \"Badge Weighted Seeding\" system to adjust a player's rank based \
        on the amount of badges they own.\n\
        Instead of considering a player's global rank at face value, tourneys calculate \
        the player's bws value and use that to determine if they are allowed to \
        participate based on the rank restrictions.\n\
        There are various formulas around but this command uses `rank^(0.9937^(badges^2))`.";

    MyCommand::new("bws", description)
        .help(help)
        .options(vec![name, rank, badges, discord])
}
