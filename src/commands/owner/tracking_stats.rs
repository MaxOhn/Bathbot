use crate::{
    embeds::{EmbedData, TrackingStatsEmbed},
    tracking::TrackingStats,
    util::MessageExt,
    Args, BotResult, Context,
};

use std::sync::Arc;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
};
use twilight_model::channel::Message;

#[command]
#[short_desc("Display stats about osu!tracking")]
#[owner()]
async fn trackingstats(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let stats = ctx.tracking().stats().await;
    if let Err(why) = create_tracked_file(&stats).await {
        error!("Error while creating tracked file: {}", why);
    }
    let embed = TrackingStatsEmbed::new(stats).build().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}

async fn create_tracked_file(stats: &TrackingStats) -> BotResult<()> {
    let file = File::create("tracked_users.txt").await?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(format!("Users ({}):\n", stats.users.len()).as_bytes())
        .await?;
    for (id, mode) in stats.users.iter() {
        if stats.queue.iter().all(|(i, m)| i != id && m != mode) {
            writer.write(b"> ").await?;
        }
        writer
            .write_all(format!("{} | {}\n", mode, id).as_bytes())
            .await?;
    }
    writer
        .write_all(format!("\n-----\n\nQueue ({}):\n", stats.queue.len()).as_bytes())
        .await?;
    for (id, mode) in stats.queue.iter() {
        if stats.users.iter().all(|(i, m)| i != id && m != mode) {
            writer.write(b"> ").await?;
        }
        writer
            .write_all(format!("{} | {}\n", mode, id).as_bytes())
            .await?;
    }
    writer.flush().await?;
    Ok(())
}
