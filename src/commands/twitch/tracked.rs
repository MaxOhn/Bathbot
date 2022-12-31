use std::{fmt::Write, sync::Arc};

use command_macros::command;
use eyre::Result;

use crate::{
    core::commands::CommandOrigin,
    util::{builder::MessageBuilder, constants::GENERAL_ISSUE},
    Context,
};

#[command]
#[desc("List all streams that are tracked in a channel")]
#[alias("tracked")]
#[group(Twitch)]
async fn prefix_trackedstreams(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    tracked(ctx, msg.into()).await
}

pub async fn tracked(ctx: Arc<Context>, orig: CommandOrigin<'_>) -> Result<()> {
    let twitch_ids = ctx.tracked_users_in(orig.channel_id());

    let mut twitch_users: Vec<_> = match ctx.client().get_twitch_users(&twitch_ids).await {
        Ok(users) => users.into_iter().map(|user| user.display_name).collect(),
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

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
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}
