use crate::{
    arguments::{Args, BwsArgs, RankRange},
    embeds::{BWSEmbed, EmbedData},
    util::{
        constants::{OSU_API_ISSUE, OSU_WEB_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::model::GameMode;
use std::{cmp::Ordering, sync::Arc};
use twilight_model::channel::Message;

#[command]
#[short_desc("Show the badge weighted seeding for a player")]
#[long_desc(
    "Show the badge weighted seeding for a player. \n\
    The current formula is `rank^(0.9937^(badges^2))`.\n\
    Next to the player's username, you can specify a rank \
    either as number or as range of the form `a..b` to check \
    the bws within that range.\n\
    This command considers __all__ current badges of a user."
)]
#[usage("[username] [number..number]")]
#[example("badewanne3", "badewanne3 42..1000", "badewanne3 1234567")]
async fn bws(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let mode = GameMode::STD;
    let args = BwsArgs::new(&ctx, args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };
    let user = match ctx.osu().user(name.as_str()).mode(mode).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    let rank_range = match args.rank_range {
        Some(RankRange::Single(rank)) => match rank.cmp(&user.pp_rank) {
            Ordering::Less => Some((rank, user.pp_rank)),
            Ordering::Greater => Some((user.pp_rank, rank)),
            Ordering::Equal => None,
        },
        Some(RankRange::Range(min, max)) => Some((min, max)),
        None => None,
    };
    let profile_fut = ctx
        .clients
        .custom
        .get_osu_profile(user.user_id, mode, false);
    let profile = match profile_fut.await {
        Ok((profile, _)) => profile,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_WEB_ISSUE).await;
            return Err(why.into());
        }
    };
    let badges = profile.badges.len();
    let embed = BWSEmbed::new(user, badges, rank_range)
        .build_owned()
        .build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}
