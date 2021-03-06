use crate::{util::MessageExt, StreamTracks, TwitchUsers};

use itertools::Itertools;
use rayon::prelude::*;
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[description = "List all streams that are tracked in this channel"]
#[aliases("tracked")]
async fn trackedstreams(ctx: &Context, msg: &Message) -> CommandResult {
    let mut twitch_users: Vec<_> = {
        let data = ctx.data.read().await;
        let twitch_users = data.get::<TwitchUsers>().unwrap();
        let tracks = data.get::<StreamTracks>().unwrap();
        twitch_users
            .par_iter()
            .filter(|(_, &twitch_id)| {
                tracks
                    .iter()
                    .any(|track| track.user_id == twitch_id && track.channel_id == msg.channel_id.0)
            })
            .map(|(name, _)| name.clone())
            .collect()
    };
    twitch_users.sort();
    let user_str = if twitch_users.is_empty() {
        "None".to_string()
    } else {
        twitch_users.into_iter().join("`, `")
    };

    // Sending the msg
    msg.channel_id
        .say(
            ctx,
            format!("Tracked twitch streams in this channel:\n`{}`", user_str),
        )
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}
