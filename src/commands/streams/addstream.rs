use crate::{
    commands::checks::*,
    database::{Platform, StreamTrack},
    util::discord,
    MySQL, StreamTracks, Twitch, TwitchUsers,
};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[checks(Authority)]
#[description = "Let me notify this channel whenever the given stream comes online"]
#[aliases("streamadd")]
#[usage = "[twitch / mixer] [stream name]"]
#[example = "twitch loltyler1"]
async fn addstream(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the platform and stream name
    let (platform, name) = if args.len() < 2 {
        msg.channel_id
            .say(
                &ctx.http,
                "The first argument must be either `twitch` or `mixer`. \
             The next argument must be the name of the stream.",
            )
            .await?;
        return Ok(());
    } else {
        let platform = match args.single::<String>()?.to_lowercase().as_str() {
            "twitch" => Platform::Twitch,
            "mixer" => Platform::Mixer,
            _ => {
                msg.channel_id
                    .say(
                        &ctx.http,
                        "The first argument must be either `twitch` or `mixer`. \
                     The next argument must be the name of the stream.",
                    )
                    .await?;
                return Ok(());
            }
        };
        let name = args.single::<String>()?.to_lowercase();
        match platform {
            Platform::Mixer => (platform, "TODO".to_string()),
            Platform::Twitch => {
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
                        match mysql.add_twitch_user(twitch_id, &name) {
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
                let track = StreamTrack::new(msg.channel_id.0, twitch_id, platform);
                if stream_tracks.insert(track) {
                    let mysql = data.get::<MySQL>().unwrap();
                    match mysql.add_stream_track(msg.channel_id.0, twitch_id, platform) {
                        Ok(_) => debug!("Inserted into stream_tracks table"),
                        Err(why) => warn!("Error while adding stream track: {}", why),
                    }
                }
                (platform, name)
            }
        }
    };

    // Sending the msg
    let response = if platform == Platform::Mixer {
        msg.channel_id
            .say(&ctx.http, "Mixer is not yet supported, soon:tm:")
            .await?
    } else {
        msg.channel_id
            .say(
                &ctx.http,
                format!(
                    "I'm now tracking `{}`'s {:?} stream in this channel",
                    name, platform
                ),
            )
            .await?
    };

    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}
