use crate::{
    util::{constants::DARK_GREEN, MessageExt},
    Args, BotResult, Context,
};

use rand::Rng;
use std::sync::Arc;
use twilight_embed_builder::builder::EmbedBuilder;
use twilight_model::channel::Message;

#[command]
#[short_desc("Get a random number")]
#[long_desc(
    "Get a random number.\n\
    If no upper limit is specified, it defaults to 100."
)]
#[usage("[upper limit]")]
async fn roll(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let num = {
        let upper_limit: u64 = args
            .next()
            .map(|arg| arg.parse().ok())
            .flatten()
            .unwrap_or(100);
        rand::thread_rng().gen_range(1, (upper_limit + 1).max(2))
    };

    let content = format!("<@{}> rolls {} point(s)", msg.author.id, num);

    let embed = EmbedBuilder::new()
        .color(DARK_GREEN)?
        .description(content)?
        .build()?;
    ctx.http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?
        .reaction_delete(&ctx, msg.author.id);
    Ok(())
}
