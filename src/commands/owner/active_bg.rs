use crate::{
    util::{matcher, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::{fmt::Write, sync::Arc};
use twilight_model::id::ChannelId;

#[command]
#[short_desc("Display active bg games")]
#[long_desc(
    "Display active bg games.\n\
    If the first argument is a channel id, \
    I will remove the bg game of that channel."
)]
#[usage("[channel id]")]
#[owner()]
async fn activebg(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (msg, mut args) = match data {
        CommandData::Message { msg, args, .. } => (msg, args),
        CommandData::Interaction { .. } => unreachable!(),
    };

    match args.next().and_then(matcher::get_mention_channel) {
        Some(channel) => match ctx.stop_game(ChannelId(channel)).await {
            Ok(true) => {
                let builder = MessageBuilder::new().content("Game stopped");
                msg.create_message(&ctx, builder).await?;
            }
            Ok(false) => {
                let builder = MessageBuilder::new().content("No game in that channel");
                msg.create_message(&ctx, builder).await?;
            }
            Err(why) => return msg.error(&ctx, why.to_string()).await,
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
            let builder = MessageBuilder::new().content(content);
            msg.create_message(&ctx, builder).await?;
        }
    }

    Ok(())
}
