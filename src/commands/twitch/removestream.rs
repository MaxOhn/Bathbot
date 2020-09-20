use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[authority()]
#[short_desc("Stop tracking a twitch user in a channel")]
#[aliases("streamremove", "untrackstream")]
#[usage("[stream name]")]
#[example("loltyler1")]
async fn removestream(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    // Parse the stream name
    if args.is_empty() {
        let content = "The first argument must be the name of the stream";
        return msg.error(&ctx, content).await;
    }
    let name = args.single::<String>().unwrap().to_lowercase();
    let twitch = &ctx.clients.twitch;
    let twitch_id = match twitch.get_user(&name).await {
        Ok(user) => user.user_id,
        Err(_) => {
            let content = format!("Twitch user `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
    };
    let channel = msg.channel_id.0;
    ctx.remove_tracking(twitch_id, channel);
    match ctx.psql().remove_stream_track(channel, twitch_id).await {
        Ok(true) => {
            debug!(
                "No longer tracking {}'s twitch for channel {}",
                name, channel
            );
            let content = format!(
                "I'm no longer tracking `{}`'s twitch stream in this channel",
                name
            );
            msg.respond(&ctx, content).await
        }
        Ok(false) => {
            let content = format!("Twitch user `{}` was not tracked in this channel", name);
            msg.error(&ctx, content).await
        }
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            Err(why)
        }
    }
}
