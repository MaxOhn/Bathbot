use crate::{
    embeds::{CacheEmbed, EmbedData},
    util::{constants::GENERAL_ISSUE, MessageExt},
    Args, BotResult, Context,
};

use std::sync::Arc;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
};
use twilight::model::channel::Message;

#[command]
#[short_desc("Display stats about the internal cache")]
#[owner()]
async fn cache(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    if let Err(why) = create_id_file(&ctx).await {
        let _ = msg.error(&ctx, GENERAL_ISSUE).await;
        return Err(why);
    }
    let embed = CacheEmbed::new(&ctx).build().build()?;
    let content = "File with cached user ids is ready";
    msg.build_response(&ctx, |m| m.content(content)?.embed(embed))
        .await?;
    Ok(())
}

async fn create_id_file(ctx: &Context) -> BotResult<()> {
    let file = File::create("cached_users.txt").await?;
    let mut writer = BufWriter::new(file);
    let user_ids: Vec<_> = ctx.cache.users.iter().map(|guard| *guard.key()).collect();
    for id in user_ids {
        writer.write_all(format!("{}\n", id).as_bytes()).await?;
    }
    writer.flush().await?;
    Ok(())
}
