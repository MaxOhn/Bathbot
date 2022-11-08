use command_macros::command;
use eyre::Result;

use crate::{
    core::commands::CommandOrigin,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, TWITCH_API_ISSUE},
        ChannelExt, CowUtils,
    },
    Context,
};

use std::sync::Arc;

#[command]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[desc("Notifying a channel when a twitch stream comes online")]
#[aliases("streamadd", "trackstream")]
#[usage("[stream name]")]
#[example("loltyler1")]
#[group(Twitch)]
async fn prefix_addstream(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
    let name = match args.next() {
        Some(arg) => arg.cow_to_ascii_lowercase(),
        None => {
            let content = "The first argument must be the name of the stream";
            msg.error(&ctx, content).await?;

            return Ok(());
        }
    };

    addstream(ctx, msg.into(), name.as_ref()).await
}

pub async fn addstream(ctx: Arc<Context>, orig: CommandOrigin<'_>, name: &'_ str) -> Result<()> {
    let twitch_id = match ctx.client().get_twitch_user(name).await {
        Ok(Some(user)) => user.user_id,
        Ok(None) => {
            let content = format!("Twitch user `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, TWITCH_API_ISSUE).await;

            return Err(err.wrap_err("failed to get twitch user"));
        }
    };

    let channel = orig.channel_id();
    ctx.add_tracking(twitch_id, channel);

    match ctx.twitch().track(channel, twitch_id).await {
        Ok(true) => {
            let content = format!("I'm now tracking `{name}`'s twitch stream in this channel");
            let builder = MessageBuilder::new().content(content);

            trace!("Now tracking twitch stream {name} for channel {channel}");

            orig.create_message(&ctx, &builder).await?;

            Ok(())
        }
        Ok(false) => {
            let content = format!("Twitch user `{name}` is already being tracked in this channel");

            orig.error(&ctx, content).await
        }
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            Err(err.wrap_err("failed to add stream track"))
        }
    }
}
