use crate::{database::Platform, util::discord, StreamTracks, TwitchUsers};

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
async fn trackedstreams(ctx: &mut Context, msg: &Message) -> CommandResult {
    let mut twitch_users: Vec<_> = {
        let data = ctx.data.read().await;
        let twitch_users = data
            .get::<TwitchUsers>()
            .expect("Could not get TwitchUsers");
        let tracks = data
            .get::<StreamTracks>()
            .expect("Could not get StreamTracks");
        twitch_users
            .par_iter()
            .filter(|(_, &twitch_id)| {
                tracks.iter().any(|track| {
                    track.user_id == twitch_id
                        && track.channel_id == msg.channel_id.0
                        && track.platform == Platform::Twitch
                })
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
    let response = msg
        .channel_id
        .say(
            &ctx.http,
            format!(
                "Tracked streams in this channel:\n\
            Twitch: `{}`\n\
            Mixer: `None`",
                user_str
            ),
        )
        .await?;

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone()).await;
    Ok(())
}
