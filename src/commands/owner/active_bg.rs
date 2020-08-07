use crate::{
    util::{matcher, MessageExt},
    Args, BotResult, Context,
};

use std::{fmt::Write, sync::Arc};
use twilight::model::{channel::Message, id::ChannelId};

#[command]
#[short_desc("Display active bg games")]
#[long_desc(
    "Display active bg games.\n\
    If the first argument is a channel id, \
    I will remove the bg game of that channel."
)]
#[usage("[channel id]")]
#[owner()]
async fn activebg(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    match args.next().and_then(matcher::get_mention_channel) {
        Some(channel) => match ctx.stop_and_remove_game(ChannelId(channel)).await {
            Ok(true) => msg.respond(&ctx, "Game stopped").await,
            Ok(false) => msg.respond(&ctx, "No game in that channel").await,
            Err(why) => msg.error(&ctx, why.to_string()).await,
        },
        None => {
            let channels = ctx.game_channels();
            let mut content = String::with_capacity(channels.len() * 20 + 20);
            content.push_str("Active games in:\n```\n");
            let mut iter = channels.into_iter();
            if let Some(first) = iter.next() {
                let _ = write!(content, "{}", first);
                for id in iter {
                    let _ = write!(content, ", {}", id);
                }
            } else {
                content.push_str("None active");
            }
            content.push_str("\n```");
            msg.respond(&ctx, content).await
        }
    }
}
