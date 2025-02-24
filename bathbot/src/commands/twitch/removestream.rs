use bathbot_macros::command;
use bathbot_util::{
    CowUtils, MessageBuilder,
    constants::{GENERAL_ISSUE, TWITCH_API_ISSUE},
};
use eyre::Result;

use crate::{Context, core::commands::CommandOrigin, util::ChannelExt};

#[command]
#[flags(AUTHORITY, ONLY_GUILDS)]
#[desc("Stop tracking a twitch user in a channel")]
#[aliases("streamremove", "untrackstream")]
#[usage("[stream name]")]
#[example("loltyler1")]
#[group(Twitch)]
async fn prefix_removestream(msg: &Message, mut args: Args<'_>) -> Result<()> {
    let name = match args.next() {
        Some(arg) => arg.cow_to_ascii_lowercase(),
        None => {
            let content = "The first argument must be the name of the stream";
            msg.error(content).await?;

            return Ok(());
        }
    };

    removestream(msg.into(), name.as_ref()).await
}

pub async fn removestream(orig: CommandOrigin<'_>, name: &'_ str) -> Result<()> {
    let twitch_id = match Context::client().get_twitch_user(name).await {
        Ok(Some(user)) => user.user_id,
        Ok(None) => {
            let content = format!("Twitch user `{name}` was not found");
            orig.error(content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = orig.error(TWITCH_API_ISSUE).await;

            return Err(err.wrap_err("failed to get twitch user"));
        }
    };

    let channel = orig.channel_id();
    Context::remove_tracking(twitch_id, channel.get());

    match Context::twitch().untrack(channel, twitch_id).await {
        Ok(true) => {
            trace!("No longer tracking {name}'s twitch for channel {channel}");

            let content =
                format!("I'm no longer tracking `{name}`'s twitch stream in this channel");

            let builder = MessageBuilder::new().embed(content);
            orig.create_message(builder).await?;

            Ok(())
        }
        Ok(false) => {
            let content = format!("Twitch user `{name}` was not tracked in this channel");

            orig.error(content).await
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            Err(err.wrap_err("failed to remove stream track"))
        }
    }
}
