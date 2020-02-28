use crate::{
    database::{Platform, StreamTrack},
    util::globals::DATABASE_ISSUE,
    MySQL, StreamTracks, Twitch, TwitchUsers,
};

use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use tokio::runtime::Runtime;

#[command]
#[description = "Calculate what score a user is missing to reach the given total pp amount"]
#[aliases("streamadd")]
#[usage = "twitch/mixer [stream name]"]
fn addstream(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse the platform and stream name
    let (platform, name) = match args.len() {
        0 | 1 => {
            msg.channel_id.say(
                &ctx.http,
                "The first argument must be either `twitch` or `mixer`. \
                 The next argument must be the name of the stream.",
            )?;
            return Ok(());
        }
        _ => {
            let platform = match args.single::<String>()?.to_lowercase().as_str() {
                "twitch" => Platform::Twitch,
                "mixer" => Platform::Mixer,
                _ => {
                    msg.channel_id.say(
                        &ctx.http,
                        "The first argument must be either `twitch` or `mixer`. \
                         The next argument must be the name of the stream.",
                    )?;
                    return Ok(());
                }
            };
            let name = args.single::<String>()?.to_lowercase();
            {
                // TODO: Distinguish between twitch and mixer
                let (twitch_id, insert) = {
                    let data = ctx.data.read();
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
                            Err(why) => {
                                msg.channel_id.say(&ctx.http, DATABASE_ISSUE)?;
                                return Err(CommandError::from(why.to_string()));
                            }
                        };
                        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
                        if let Err(why) = mysql.add_twitch_user(twitch_id, &name) {
                            warn!("Error while adding twitch user: {}", why);
                        }
                        (twitch_id, true)
                    }
                };
                let mut data = ctx.data.write();
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
            }
            (platform, name)
        }
    };

    // Creating the embed
    let _ = msg.channel_id.say(
        &ctx.http,
        format!(
            "I'm now tracking {}'s {:?} stream in this channel",
            name, platform
        ),
    );
    Ok(())
}
