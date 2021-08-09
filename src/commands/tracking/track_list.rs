use crate::{
    embeds::{EmbedData, TrackListEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use futures::{
    future::FutureExt,
    stream::{FuturesUnordered, StreamExt},
};
use rosu_v2::prelude::OsuError;
use std::sync::Arc;

#[command]
#[authority()]
#[short_desc("Display tracked users of a channel")]
#[aliases("tl")]
pub async fn tracklist(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let channel_id = data.channel_id();
    let tracked = ctx.tracking().list(channel_id);
    let count = tracked.len();

    let mut user_futs: FuturesUnordered<_> = tracked
        .into_iter()
        .map(|(user_id, mode, limit)| {
            ctx.osu()
                .user(user_id)
                .mode(mode)
                .map(move |result| (user_id, mode, limit, result))
        })
        .collect();

    let mut users = Vec::with_capacity(count);

    while let Some((user_id, mode, limit, result)) = user_futs.next().await {
        match result {
            Ok(user) => users.push((user.username, mode, limit)),
            Err(OsuError::NotFound) => {
                let remove_fut = ctx.tracking().remove_user(user_id, channel_id, ctx.psql());

                if let Err(why) = remove_fut.await {
                    warn!(
                        "Error while removing unknown user ({},{}) from tracking: {}",
                        user_id, mode, why
                    );
                }
            }
            Err(why) => {
                let _ = data.error(&ctx, OSU_API_ISSUE).await;

                return Err(why.into());
            }
        }
    }

    drop(user_futs);

    users.sort_unstable_by(|(u1, m1, _), (u2, m2, _)| {
        (*m1 as u8).cmp(&(*m2 as u8)).then(u1.cmp(u2))
    });

    let embeds = TrackListEmbed::new(users);

    if embeds.is_empty() {
        let content = "No tracked users in this channel";
        let builder = MessageBuilder::new().content(content);
        data.create_message(&ctx, builder).await?;
    } else {
        for embed_data in embeds {
            let embed = embed_data.into_builder().build();
            let builder = MessageBuilder::new().embed(embed);
            data.create_message(&ctx, builder).await?;
        }
    }

    Ok(())
}
