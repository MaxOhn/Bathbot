use crate::{
    arguments::{Args, NameIntArgs},
    embeds::{BWSEmbed, EmbedData},
    util::{
        constants::{OSU_API_ISSUE, OSU_WEB_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::models::GameMode;
use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Show the badge weighted seeding for a player")]
#[long_desc(
    "Show the badge weighted seeding for a player. \n\
    The current formula is `rank^(0.9937^(badges^2))`.\n\
    Next to the player's username, you can specify a rank \
    to check how the bws would change towards that rank.\n\
    This command considers __all__ current badges of a user."
)]
#[usage("[username] [rank]")]
#[example("badewanne3", "badewanne3 42", "badewanne3 1234567")]
async fn bws(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let mode = GameMode::STD;
    let args = NameIntArgs::new(&ctx, args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };
    if let Some(true) = args.number.map(|n| n == 0) {
        let content = "Given rank must be positive";
        return msg.error(&ctx, content).await;
    }
    let user = match ctx.osu_user(&name, mode).await {
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
    let rank = match args.number == Some(user.pp_rank) {
        true => None,
        false => args.number,
    };
    let profile_fut = ctx
        .clients
        .custom
        .get_osu_profile(user.user_id, mode, false);
    let profile = match profile_fut.await {
        Ok((profile, _)) => profile,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_WEB_ISSUE).await;
            return Err(why);
        }
    };
    let badges = profile.badges.len();
    let embed = BWSEmbed::new(user, badges, rank).build().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}
