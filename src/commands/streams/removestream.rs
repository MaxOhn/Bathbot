use crate::{
    commands::checks::*, database::StreamTrack, util::MessageExt, MySQL, StreamTracks, TwitchUsers,
};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[checks(Authority)]
#[description = "Let me no longer notify this channel when the given twitch stream comes online"]
#[aliases("streamremove")]
#[usage = "[stream name]"]
#[example = "loltyler1"]
async fn removestream(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the platform and stream name
    let result = if args.is_empty() {
        msg.channel_id
            .say(ctx, "The first argument must be the name of the stream.")
            .await?
            .reaction_delete(ctx, msg.author.id)
            .await;
        return Ok(());
    } else {
        let name = args.single::<String>()?.to_lowercase();
        let data = ctx.data.read().await;
        let twitch_users = data.get::<TwitchUsers>().unwrap();
        if twitch_users.contains_key(&name) {
            let twitch_id = *twitch_users.get(&name).unwrap();
            std::mem::drop(data);
            let mut data = ctx.data.write().await;
            let stream_tracks = data.get_mut::<StreamTracks>().unwrap();
            let track = StreamTrack::new(msg.channel_id.0, twitch_id);
            if stream_tracks.remove(&track) {
                let mysql = data.get::<MySQL>().unwrap();
                if let Err(why) = mysql.remove_stream_track(msg.channel_id.0, twitch_id).await {
                    warn!("Error while removing stream track: {}", why);
                }
            }
            Some(name)
        } else {
            None
        }
    };
    let content = if let Some(name) = result {
        format!(
            "I'm no longer tracking `{}`'s twitch stream in this channel",
            name
        )
    } else {
        "That stream wasn't tracked anyway".to_string()
    };

    // Sending the msg
    msg.channel_id
        .say(ctx, content)
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}
