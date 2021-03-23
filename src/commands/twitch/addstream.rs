use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    Args, BotResult, Context,
};

use cow_utils::CowUtils;
use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[authority()]
#[short_desc("Notifying a channel when a twitch stream comes online")]
#[aliases("streamadd", "trackstream")]
#[usage("[stream name]")]
#[example("loltyler1")]
async fn addstream(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    // Parse the stream name
    let name = match args.next() {
        Some(arg) => arg.cow_to_lowercase(),
        None => {
            let content = "The first argument must be the name of the stream";

            return msg.error(&ctx, content).await;
        }
    };

    let twitch = &ctx.clients.twitch;

    let twitch_id = match twitch.get_user(&name).await {
        Ok(user) => user.user_id,
        Err(_) => {
            let content = format!("Twitch user `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
    };

    let channel = msg.channel_id.0;
    ctx.add_tracking(twitch_id, channel);

    match ctx.psql().add_stream_track(channel, twitch_id).await {
        Ok(true) => {
            let content = format!(
                "I'm now tracking `{}`'s twitch stream in this channel",
                name
            );

            debug!(
                "Now tracking twitch stream {} for channel {}",
                name, msg.channel_id
            );

            msg.send_response(&ctx, content).await
        }
        Ok(false) => {
            let content = format!(
                "Twitch user `{}` is already being tracked in this channel",
                name
            );

            msg.error(&ctx, content).await
        }
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            Err(why)
        }
    }
}
