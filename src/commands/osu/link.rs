use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Link your discord account to an osu name")]
#[long_desc(
    "Link your discord account to an osu name. \
     If no arguments are provided, I will unlink \
     your discord account from any osu name."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[example("\"nathan on osu\"")]
async fn link(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let id = *msg.author.id.as_u64();
    if args.is_empty() {
        {
            let mut data = ctx.data.write().await;
            let links = data.get_mut::<DiscordLinks>().unwrap();
            links.remove_entry(&id);
        }
        {
            let data = ctx.data.read().await;
            let mysql = data.get::<MySQL>().unwrap();
            if let Err(why) = mysql.remove_discord_link(id).await {
                msg.respond(&ctx, GENERAL_ISSUE).await?;
                return Err(why);
            }
        }
        msg.respond(&ctx, "You are no longer linked").await?;
        Ok(())
    } else {
        let name = args.single::<String>()?;
        {
            let mut data = ctx.data.write().await;
            let links = data.get_mut::<DiscordLinks>().unwrap();
            let value = links.entry(id).or_insert_with(String::default);
            *value = name.clone();
        }
        {
            let data = ctx.data.read().await;
            let mysql = data.get::<MySQL>().unwrap();
            match mysql.add_discord_link(id, &name).await {
                Ok(_) => debug!("Discord user {} now linked to osu name {} in DB", id, name),
                Err(why) => {
                    msg.respond(&ctx, GENERAL_ISSUE).await?;
                    return Err(why);
                }
            }
        }
        let content = format!(
            "I linked discord's `{}` with osu's `{}`",
            msg.author.name, name
        );
        msg.respond(&ctx, content).await?;
        Ok(())
    }
}
