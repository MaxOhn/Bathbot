use crate::{util::MessageExt, Args, BotResult, Context};

use std::{fmt::Write, sync::Arc};
use twilight::model::channel::Message;

#[command]
#[short_desc("List all streams that are tracked in a channel")]
#[aliases("tracked")]
async fn trackedstreams(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let channel = msg.channel_id.0;
    let twitch_ids: Vec<_> = ctx
        .data
        .tracked_streams
        .read()
        .await
        .iter()
        .filter_map(|(user, channels)| {
            if channels.contains(&channel) {
                Some(*user)
            } else {
                None
            }
        })
        .collect();
    let twitch = &ctx.clients.twitch;
    let mut twitch_users: Vec<_> = match twitch.get_users(&twitch_ids).await {
        Ok(users) => users.into_iter().map(|user| user.display_name).collect(),
        Err(why) => {
            let content = "Error while retrieving twitch users";
            msg.respond(&ctx, content).await?;
            return Err(why.into());
        }
    };
    twitch_users.sort_unstable_by(|a, b| a.cmp(&b));
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
