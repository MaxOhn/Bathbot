use crate::{
    arguments::{Args, NameArgs},
    embeds::{AvatarEmbed, EmbedData},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu::backend::UserRequest;
use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Displaying someone's osu profile picture")]
#[aliases("pfp")]
#[usage("[username]")]
#[example("Badewanne3")]
async fn avatar(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let name = match NameArgs::new(args).name {
        Some(name) => name,
        None => {
            return msg.error(&ctx, "You must specify a username").await;
        }
    };
    let user = {
        let req = UserRequest::with_username(&name);
        match req.queue_single(ctx.osu()).await {
            Ok(user) => match user {
                Some(user) => user,
                None => {
                    let content = format!("User `{}` was not found", name);
                    return msg.error(&ctx, content).await;
                }
            },
            Err(why) => {
                msg.error(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        }
    };
    let embed = AvatarEmbed::new(user).build().build();
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}
