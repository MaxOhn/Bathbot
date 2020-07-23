use crate::{
    arguments::Args,
    bail,
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, Context,
};

use std::sync::Arc;
use tokio::time::{timeout, Duration};
use twilight::model::channel::Message;

#[command]
#[authority()]
#[short_desc("Stop tracking a twitch user in a channel")]
#[aliases("streamremove", "untrackstream")]
#[usage("[stream name]")]
#[example("loltyler1")]
async fn removestream(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let mut args = Args::new(msg.content.clone());
    // Parse the stream name
    if args.is_empty() {
        let content = "The first argument must be the name of the stream";
        msg.respond(&ctx, content).await?;
        return Ok(());
    }
    let name = args.single::<String>().unwrap().to_lowercase();
    let twitch = &ctx.clients.twitch;
    let twitch_id = match twitch.get_user(&name).await {
        Ok(user) => user.user_id,
        Err(_) => {
            let content = format!("Twitch user `{}` was not found", name);
            msg.respond(&ctx, content).await?;
            return Ok(());
        }
    };
    let channel = msg.channel_id.0;
    {
        let mut tracked_streams =
            match timeout(Duration::from_secs(10), ctx.tracked_streams.write()).await {
                Ok(tracks) => tracks,
                Err(_) => {
                    msg.respond(&ctx, GENERAL_ISSUE).await?;
                    bail!("Timed out while waiting for write access");
                }
            };
        tracked_streams.entry(twitch_id).and_modify(|channels| {
            if let Some(idx) = channels.iter().position(|&id| id == channel) {
                channels.remove(idx);
            };
        });
    }
    let psql = &ctx.clients.psql;
    if let Err(why) = psql.remove_stream_track(channel, twitch_id).await {
        msg.respond(&ctx, GENERAL_ISSUE).await?;
        bail!("Error while removing stream track from DB: {}", why);
    }
    debug!(
        "No longer tracking {}'s twitch for channel {}",
        name, channel
    );
    let content = format!(
        "I'm no longer tracking `{}`'s twitch stream in this channel",
        name
    );
    msg.respond(&ctx, content).await?;
    Ok(())
}
