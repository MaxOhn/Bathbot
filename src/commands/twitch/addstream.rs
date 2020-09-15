use crate::{util::MessageExt, Args, BotResult, Context};

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
    ctx.add_tracking(twitch_id, channel);
    if let Err(why) = ctx.psql().add_stream_track(channel, twitch_id).await {
        error!("Error while inserting stream track into DB: {}", why);
    }

    // Sending the msg
    let content = format!(
        "I'm now tracking `{}`'s twitch stream in this channel",
        name
    );
    debug!(
        "Now tracking twitch stream {} for channel {}",
        name, msg.channel_id
    );
    msg.respond(&ctx, content).await?;
    Ok(())
}
