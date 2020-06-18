use crate::{
    embeds::{AboutEmbed, EmbedData},
    util::MessageExt,
};

use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::channel::Message,
    prelude::Context,
};

#[command]
#[description = "Displaying some information about this bot"]
#[aliases("info")]
async fn about(ctx: &Context, msg: &Message) -> CommandResult {
    let data = match AboutEmbed::new(ctx).await {
        Ok(data) => data,
        Err(why) => {
            msg.channel_id
                .say(ctx, "Some issue while calculating about data, blame bade")
                .await?
                .reaction_delete(ctx, msg.author.id)
                .await;
            return Err(why.to_string().into());
        }
    };
    msg.channel_id
        .send_message(ctx, |m| m.embed(|e| data.build(e)))
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}
