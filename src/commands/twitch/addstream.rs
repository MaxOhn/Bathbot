use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;

#[command]
#[authority()]
#[short_desc("Notifying a channel when a twitch stream comes online")]
#[aliases("streamadd", "trackstream")]
#[usage("[stream name]")]
#[example("loltyler1")]
async fn addstream(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { mut args, msg, num } => match super::StreamArgs::args(&mut args) {
            Ok(name) => {
                _addstream(ctx, CommandData::Message { msg, args, num }, name.as_ref()).await
            }
            Err(content) => {
                let builder = MessageBuilder::new().content(content);
                msg.create_message(&ctx, builder).await?;

                return Ok(());
            }
        },
        CommandData::Interaction { command } => super::slash_trackstream(ctx, command).await,
    }
}

pub async fn _addstream(ctx: Arc<Context>, data: CommandData<'_>, name: &'_ str) -> BotResult<()> {
    let twitch = &ctx.clients.twitch;

    let twitch_id = match twitch.get_user(name).await {
        Ok(user) => user.user_id,
        Err(_) => {
            let content = format!("Twitch user `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
    };

    let channel = data.channel_id().0;
    ctx.add_tracking(twitch_id, channel);

    match ctx.psql().add_stream_track(channel, twitch_id).await {
        Ok(true) => {
            let content = format!(
                "I'm now tracking `{}`'s twitch stream in this channel",
                name
            );

            let builder = MessageBuilder::new().content(content);

            debug!(
                "Now tracking twitch stream {} for channel {}",
                name, channel
            );

            data.create_message(&ctx, builder).await?;

            Ok(())
        }
        Ok(false) => {
            let content = format!(
                "Twitch user `{}` is already being tracked in this channel",
                name
            );

            data.error(&ctx, content).await
        }
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            Err(why)
        }
    }
}
