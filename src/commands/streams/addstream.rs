use crate::{
    commands::checks::*, database::StreamTrack, util::MessageExt, MySQL, StreamTracks, Twitch,
    TwitchUsers,
};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[checks(Authority)]
#[description = "Let me notify this channel whenever the given twitch stream comes online"]
#[aliases("streamadd")]
#[usage = "[stream name]"]
#[example = "loltyler1"]
async fn addstream(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the platform and stream name
    let name = if args.is_empty() {
        msg.channel_id
            .say(ctx, "The first argument must be the name of the stream.")
            .await?
            .reaction_delete(ctx, msg.author.id)
            .await;
        return Ok(());
    } else {
        let name = args.single::<String>()?.to_lowercase();
        let (twitch_id, insert) = {
            let data = ctx.data.read().await;
            let twitch_users = data.get::<TwitchUsers>().unwrap();
            if twitch_users.contains_key(&name) {
                (*twitch_users.get(&name).unwrap(), false)
            } else {
                let twitch = data.get::<Twitch>().unwrap();
                let twitch_id = match twitch.get_user(&name).await {
                    Ok(user) => user.user_id,
                    Err(_) => {
                        msg.channel_id
                            .say(&ctx.http, format!("Twitch user `{}` was not found", name))
                            .await?;
                        return Ok(());
                    }
                };
                let mysql = data.get::<MySQL>().unwrap();
                match mysql.add_twitch_user(twitch_id, &name).await {
                    Ok(_) => debug!("Inserted into twitch_users table"),
                    Err(why) => warn!("Error while adding twitch user: {}", why),
                }
                (twitch_id, true)
            }
        };
        let mut data = ctx.data.write().await;
        if insert {
            let twitch_users = data.get_mut::<TwitchUsers>().unwrap();
            twitch_users.insert(name.clone(), twitch_id);
        }
        let stream_tracks = data.get_mut::<StreamTracks>().unwrap();
        let track = StreamTrack::new(msg.channel_id.0, twitch_id);
        if stream_tracks.insert(track) {
            let mysql = data.get::<MySQL>().unwrap();
            match mysql.add_stream_track(msg.channel_id.0, twitch_id).await {
                Ok(_) => debug!("Inserted into stream_tracks table"),
                Err(why) => warn!("Error while adding stream track: {}", why),
            }
        }
        name
    };

    // Sending the msg
    msg.channel_id
        .say(
            ctx,
            format!(
                "I'm now tracking `{}`'s twitch stream in this channel",
                name
            ),
        )
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}
