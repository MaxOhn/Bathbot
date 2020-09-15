use crate::{bail, util::MessageExt, Args, BotResult, Context};

use std::{fmt::Write, sync::Arc};
use twilight_model::channel::Message;

#[command]
#[short_desc("List all streams that are tracked in a channel")]
#[aliases("tracked")]
async fn trackedstreams(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let twitch_ids = ctx.tracked_users_in(msg.channel_id);
    let twitch = &ctx.clients.twitch;
    let mut twitch_users: Vec<_> = match twitch.get_users(&twitch_ids).await {
        Ok(users) => users.into_iter().map(|user| user.display_name).collect(),
        Err(why) => {
            let content = "Error while retrieving twitch users";
            let _ = msg.error(&ctx, content).await;
            bail!("error while getting twitch users: {}", why);
        }
    };
    twitch_users.sort_unstable();
    let mut content = "Tracked twitch streams in this channel:\n".to_owned();
    if twitch_users.is_empty() {
        content.push_str("None");
    } else {
        let len = twitch_users.iter().map(|user| user.len() + 4).sum();
        content.reserve_exact(len);
        let mut users = twitch_users.into_iter();
        let _ = write!(content, "`{}`", users.next().unwrap());
        for user in users {
            let _ = write!(content, ", `{}`", user);
        }
    }
    msg.respond(&ctx, content).await?;
    Ok(())
}
