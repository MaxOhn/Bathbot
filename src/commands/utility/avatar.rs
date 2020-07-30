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
            msg.respond(&ctx, "You must specify a username").await?;
            return Ok(());
        }
    };
    let user = {
        let req = UserRequest::with_username(&name);
        let osu = &ctx.clients.osu;
        match req.queue_single(osu).await {
            Ok(user) => match user {
                Some(user) => user,
                None => {
                    msg.respond(&ctx, format!("User `{}` was not found", name))
                        .await?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        }
    };
    let embed = AvatarEmbed::new(user).build().build();
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}
