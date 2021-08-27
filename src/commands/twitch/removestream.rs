use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;

#[command]
#[authority()]
#[short_desc("Stop tracking a twitch user in a channel")]
#[aliases("streamremove", "untrackstream")]
#[usage("[stream name]")]
#[example("loltyler1")]
async fn removestream(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { mut args, msg, num } => match super::StreamArgs::args(&mut args) {
            Ok(name) => {
                _removestream(ctx, CommandData::Message { msg, args, num }, name.as_ref()).await
            }
            Err(content) => {
                let builder = MessageBuilder::new().content(content);
                msg.create_message(&ctx, builder).await?;

                Ok(())
            }
        },
        CommandData::Interaction { command } => super::slash_trackstream(ctx, *command).await,
    }
}

pub async fn _removestream(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    name: &'_ str,
) -> BotResult<()> {
    let twitch = &ctx.clients.twitch;

    let twitch_id = match twitch.get_user(name).await {
        Ok(user) => user.user_id,
        Err(_) => {
            let content = format!("Twitch user `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
    };

    let channel = data.channel_id().0;
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

            let builder = MessageBuilder::new().content(content);
            data.create_message(&ctx, builder).await?;

            Ok(())
        }
        Ok(false) => {
            let content = format!("Twitch user `{}` was not tracked in this channel", name);

            data.error(&ctx, content).await
        }
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            Err(why)
        }
    }
}
