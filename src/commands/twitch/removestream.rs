use crate::{
    arguments::Args,
    bail,
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, Context,
};

use std::sync::Arc;
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
    let success = ctx.tracked_streams.update(&twitch_id, |_, channels| {
        let mut new_channels = channels.clone();
        if let Some(idx) = new_channels.iter().position(|&id| id == channel) {
            new_channels.remove(idx);
        }
        new_channels
    });
    let psql = &ctx.clients.psql;
    if let Err(why) = psql.remove_stream_track(channel, twitch_id).await {
        msg.respond(&ctx, GENERAL_ISSUE).await?;
        bail!("Error while removing stream track from DB: {}", why);
    }
    if success {
        let content = format!(
            "I'm no longer tracking `{}`'s twitch stream in this channel",
            name
        );
        msg.respond(&ctx, content).await?;
    } else {
        let content = "That stream wasn't tracked in this channel anyway";
        msg.respond(&ctx, content).await?;
    };
    Ok(())
}
