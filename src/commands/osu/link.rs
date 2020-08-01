use crate::{
    bail,
    util::{constants::GENERAL_ISSUE, MessageExt},
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Link your discord to an osu profile")]
#[long_desc(
    "Link your discord account to an osu name. \n\
     Don't forget the `\"` if the name contains whitespace.\n\
     Alternatively you can substitute whitespace with `_` characters.\n\
     If no arguments are provided, I will unlink \
     your discord account from any osu name."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[example("\"nathan on osu\"")]
async fn link(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let discord_id = msg.author.id.0;
    match args.single::<String>() {
        Ok(name) => {
            if let Err(why) = ctx.add_link(discord_id, &name).await {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;
                bail!("error while adding link: {}", why);
            }
            let content = format!(
                "I linked discord's `{}` with osu's `{}`",
                msg.author.name, name
            );
            msg.respond(&ctx, content).await
        }
        Err(_) => {
            if let Err(why) = ctx.remove_link(discord_id).await {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;
                bail!("error while removing link: {}", why);
            }
            msg.respond(&ctx, "You are no longer linked").await
        }
    }
}
