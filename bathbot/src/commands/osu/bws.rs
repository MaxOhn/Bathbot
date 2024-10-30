use std::{borrow::Cow, mem};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_util::{constants::GENERAL_ISSUE, matcher, MessageBuilder, TourneyBadges};
use eyre::{Report, Result};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::{require_link, user_not_found};
use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{BWSEmbed, EmbedData},
    manager::redis::osu::{UserArgs, UserArgsError},
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "bws",
    desc = "Show the badge weighted seeding for an osu!standard player",
    help = "To combat those pesky derank players ruining everyone's tourneys, \
    many tournaments use a \"Badge Weighted Seeding\" system to adjust a player's rank based \
    on the amount of badges they own.\n\
    Instead of considering a player's global rank at face value, tourneys calculate \
    the player's bws value and use that to determine if they are allowed to \
    participate based on the rank restrictions.\n\
    There are various formulas around but this command uses `rank^(0.9937^(badges^2))`."
)]
pub struct Bws<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        min_value = 1,
        desc = "Specify a target rank to reach",
        help = "If specified, it will calculate how the bws value would evolve towards the given rank."
    )]
    rank: Option<u32>,
    #[command(
        min_value = 0,
        desc = "Specify an amount of badges to reach",
        help = "Calculate how the bws value evolves towards the given amount of badges.\n\
        If none is specified, it defaults to the current amount + 2."
    )]
    badges: Option<usize>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

impl<'m> Bws<'m> {
    fn args(args: Args<'m>) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut discord = None;
        let mut rank = None;
        let mut badges = None;

        for arg in args.take(3) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "rank" | "r" => match value.parse::<u32>() {
                        Ok(num) => rank = Some(num.max(1)),
                        Err(_) => {
                            let content = "Failed to parse `rank`. Must be a positive integer.";

                            return Err(content.into());
                        }
                    },
                    "badges" | "badge" | "b" => match value.parse() {
                        Ok(num) => badges = Some(num),
                        Err(_) => {
                            let content = "Failed to parse `badges`. Must be a positive integer.";

                            return Err(content.into());
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\nAvailable options are: `rank` or `badges`."
                        );

                        return Err(content.into());
                    }
                }
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Ok(Self {
            name,
            rank,
            badges,
            discord,
        })
    }
}

async fn slash_bws(mut command: InteractionCommand) -> Result<()> {
    let args = Bws::from_interaction(command.input_data())?;

    bws((&mut command).into(), args).await
}

#[command]
#[desc("Show the badge weighted seeding for a player")]
#[help(
    "Show the badge weighted seeding for a player. \n\
    The current formula is `rank^(0.9937^(badges^2))`.\n\
    Next to the player's username, you can specify `rank=integer` \
    to show how the bws value progresses towards that rank.\n\
    Similarly, you can specify `badges=integer` to show how the value \
    progresses towards that badge amount."
)]
#[usage("[username] [rank=integer] [badges=integer]")]
#[examples("badewanne3", "badewanne3 rank=1234 badges=10", "badewanne3 badges=3")]
#[group(Osu)]
async fn prefix_bws(msg: &Message, args: Args<'_>) -> Result<()> {
    match Bws::args(args) {
        Ok(args) => bws(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

const MIN_BADGES_OFFSET: usize = 2;

async fn bws(orig: CommandOrigin<'_>, args: Bws<'_>) -> Result<()> {
    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match Context::user_config().osu_id(orig.user_id()?).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&orig).await,
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let Bws { rank, badges, .. } = args;

    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let badges_curr =
        TourneyBadges::count(user.badges.iter().map(|badge| badge.description.as_str()));

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

    let embed_data = BWSEmbed::new(&user, badges_curr, badges_min, badges_max, rank);
    let embed = embed_data.build();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}
