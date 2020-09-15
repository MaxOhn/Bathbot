use crate::{
    embeds::{EmbedData, InviteEmbed},
    util::{constants::BATHBOT_WORKSHOP, MessageExt},
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[short_desc("Invite me to your server")]
#[aliases("inv")]
async fn invite(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let embed = InviteEmbed::new().build().build()?;
    msg.build_response(&ctx, |m| m.content(BATHBOT_WORKSHOP)?.embed(embed))
        .await?;
    Ok(())
}
