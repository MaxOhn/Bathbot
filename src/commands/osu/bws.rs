use super::request_user;
use crate::{
    arguments::{Args, BwsArgs, RankRange},
    embeds::{BWSEmbed, EmbedData},
    util::{constants::OSU_API_ISSUE, matcher::tourney_badge, MessageExt},
    BotResult, Context,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::{cmp::Ordering, sync::Arc};
use twilight_model::channel::Message;

#[command]
#[short_desc("Show the badge weighted seeding for a player")]
#[long_desc(
    "Show the badge weighted seeding for a player. \n\
    The current formula is `rank^(0.9937^(badges^2))`.\n\
    Next to the player's username, you can specify a rank \
    either as number or as range of the form `a..b` to check \
    the bws within that range."
)]
#[usage("[username] [number[..number]]")]
#[example("badewanne3", "badewanne3 42..1000", "badewanne3 1234567")]
async fn bws(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let mode = GameMode::STD;
    let args = BwsArgs::new(&ctx, args);

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    let user = match request_user(&ctx, &name, Some(mode)).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let global_rank = user.statistics.as_ref().unwrap().global_rank.unwrap_or(0);

    let rank_range = match args.rank_range {
        Some(RankRange::Single(rank)) => match rank.cmp(&global_rank) {
            Ordering::Less => Some((rank, global_rank)),
            Ordering::Greater => Some((global_rank, rank)),
            Ordering::Equal => None,
        },
        Some(RankRange::Range(min, max)) => Some((min, max)),
        None => None,
    };

    let badges = user
        .badges
        .as_ref()
        .unwrap()
        .iter()
        .filter(|badge| tourney_badge(badge.description.as_str()))
        .count();

    let embed = BWSEmbed::new(user, badges, rank_range)
        .build_owned()
        .build()?;

    msg.build_response(&ctx, |m| m.embed(embed)).await?;

    Ok(())
}
