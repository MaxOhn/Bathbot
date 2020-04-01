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
use tokio::runtime::Runtime;

#[command]
#[checks(Authority)]
#[description = "Let me notify this channel whenever the given stream comes online"]
#[aliases("streamadd")]
#[usage = "[twitch / mixer] [stream name]"]
#[example = "twitch loltyler1"]
async fn addstream(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
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
                    let twitch_users = data
                        .get::<TwitchUsers>()
                        .expect("Could not get TwitchUsers");
                    if twitch_users.contains_key(&name) {
                        (*twitch_users.get(&name).unwrap(), false)
                    } else {
                        let twitch = data.get::<Twitch>().expect("Could not get Twitch");
                        let mut rt = Runtime::new().unwrap();
                        let twitch_id = match rt.block_on(twitch.get_user(&name)) {
                            Ok(user) => user.user_id,
                            Err(_) => {
                                msg.channel_id
                                    .say(&ctx.http, format!("Twitch user `{}` was not found", name))
                                    .await?;
                                return Ok(());
                            }
                        };
                        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                        if let Err(why) = mysql.add_twitch_user(twitch_id, &name) {
                            warn!("Error while adding twitch user: {}", why);
                        }
                        (twitch_id, true)
                    }
                };
                let mut data = ctx.data.write().await;
                if insert {
                    let twitch_users = data
                        .get_mut::<TwitchUsers>()
                        .expect("Could not get TwitchUsers");
                    twitch_users.insert(name.clone(), twitch_id);
                }
                let stream_tracks = data
                    .get_mut::<StreamTracks>()
                    .expect("Could not get StreamTracks");
                let track = StreamTrack::new(msg.channel_id.0, twitch_id, platform);
                if stream_tracks.insert(track) {
                    let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                    if let Err(why) = mysql.add_stream_track(msg.channel_id.0, twitch_id, platform)
                    {
                        warn!("Error while adding stream track: {}", why);
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

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}
