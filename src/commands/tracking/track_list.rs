use crate::{
    arguments::Args,
    embeds::{EmbedData, TrackListEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use futures::future::{try_join_all, TryFutureExt};
use rosu::{backend::UserRequest, models::GameMode};
use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Display tracked users of a channel")]
#[example("tl")]
async fn tracklist(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let user_futs = ctx
        .tracking()
        .read()
        .await
        .list(msg.channel_id)
        .into_iter()
        .map(|(user_id, mode)| {
            UserRequest::with_user_id(user_id)
                .mode(mode)
                .queue_single(ctx.osu())
                .map_ok(move |user| (user_id, mode, user))
        });
    let mut users: Vec<(String, GameMode)> = match try_join_all(user_futs).await {
        Ok(users) => {
            let (found, not_found): (Vec<_>, _) =
                users.into_iter().partition(|(.., user)| user.is_some());
            for (user_id, mode, _) in not_found {
                if let Err(why) = ctx
                    .tracking()
                    .write()
                    .await
                    .remove(user_id, mode, msg.channel_id, ctx.psql())
                    .await
                {
                    warn!(
                        "Error while removing unknown user ({},{}) from tracking: {}",
                        user_id, mode, why
                    );
                }
            }
            found
                .into_iter()
                .map(|(_, mode, user)| (user.unwrap().username, mode))
                .collect()
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    users.sort_by(|(u1, m1), (u2, m2)| (*m1 as u8).cmp(&(*m2 as u8)).then(u1.cmp(&u2)));
    for data in TrackListEmbed::new(users) {
        let embed = data.build().build()?;
        msg.build_response(&ctx, |m| m.embed(embed)).await?;
    }
    Ok(())
}
