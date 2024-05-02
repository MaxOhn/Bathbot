use std::fmt::Write;

use bathbot_macros::command;
use bathbot_util::{constants::GENERAL_ISSUE, MessageBuilder};
use eyre::Result;

use crate::{core::commands::CommandOrigin, Context};

#[command]
#[desc("List all streams that are tracked in a channel")]
#[alias("tracked")]
#[group(Twitch)]
async fn prefix_trackedstreams(msg: &Message) -> Result<()> {
    tracked(msg.into()).await
}

pub async fn tracked(orig: CommandOrigin<'_>) -> Result<()> {
    let twitch_ids = Context::tracked_users_in(orig.channel_id());

    let mut twitch_users: Vec<_> = match Context::client().get_twitch_users(&twitch_ids).await {
        Ok(users) => users.into_iter().map(|user| user.display_name).collect(),
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get twitch users"));
        }
    };

    twitch_users.sort_unstable();
    let mut content = "Tracked twitch streams in this channel:\n".to_owned();
    let mut users = twitch_users.into_iter();

    if let Some(user) = users.next() {
        let _ = write!(content, "`{user}`");

        for user in users {
            let _ = write!(content, ", `{user}`");
        }
    } else {
        content.push_str("None");
    }

    let builder = MessageBuilder::new().embed(content);
    orig.create_message(builder).await?;

    Ok(())
}
