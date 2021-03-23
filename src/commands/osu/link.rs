use crate::{
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        matcher, MessageExt,
    },
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Link your discord to an osu profile")]
#[long_desc(
    "Link your discord account to an osu name. \n\
     Don't forget the `\"` if the name contains whitespace.\n\
     Alternatively you can substitute whitespace with `_` characters.\n\
     If no arguments are provided, I will unlink \
     your discord account from any osu name."
)]
#[usage("[username / url to user profile]")]
#[example("badewanne3", "\"nathan on osu\"", "https://osu.ppy.sh/users/2211396")]
async fn link(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let discord_id = msg.author.id.0;

    match args.next() {
        Some(arg) => {
            let name = match matcher::get_osu_user_id(arg) {
                Some(id) => match ctx.osu().user(id).await {
                    Ok(user) => user.username,
                    Err(why) => {
                        let _ = msg.error(&ctx, OSU_API_ISSUE).await;

                        return Err(why.into());
                    }
                },
                None => arg.to_owned(),
            };

            if name.chars().count() > 16 {
                let content = "That name is too long, must be at most 16 characters";

                return msg.error(&ctx, content).await;
            }

            if let Err(why) = ctx.add_link(discord_id, name.as_str()).await {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }

            let content = format!(
                "I linked discord's `{}` with osu's `{}`",
                msg.author.name, name
            );

            msg.send_response(&ctx, content).await
        }
        None => {
            if let Err(why) = ctx.remove_link(discord_id).await {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }

            msg.send_response(&ctx, "You are no longer linked").await
        }
    }
}
