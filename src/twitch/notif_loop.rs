use super::*;
use crate::{
    embeds::{EmbedData, TwitchNotifEmbed},
    Context,
};

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use strfmt::strfmt;
use tokio::time;
use twilight::builders::embed::EmbedBuilder;

pub async fn twitch_loop(ctx: Arc<Context>, twitch: Twitch) {
    let mut online_streams = HashSet::new();
    let mut interval = time::interval(time::Duration::from_secs(10 * 60));
    loop {
        interval.tick().await;
        let now_online = {
            // Get data about what needs to be tracked for which channel
            let stream_tracks = reading.get::<StreamTracks>().unwrap();
            let user_ids: Vec<_> = stream_tracks.iter().map(|track| track.user_id).collect();

            // Get stream data about all streams that need to be tracked
            let mut streams = match twitch.get_streams(&user_ids).await {
                Ok(streams) => streams,
                Err(why) => {
                    warn!("Error while retrieving streams: {}", why);
                    return;
                }
            };

            // Filter streams whether they're live
            streams.retain(TwitchStream::is_live);
            let now_online: HashSet<_> = streams.iter().map(|stream| stream.user_id).collect();

            // If there was no activity change since last time, don't do anything
            if now_online == online_streams {
                None
            } else {
                // Filter streams whether its already known they're live
                streams.retain(|stream| !online_streams.contains(&stream.user_id));

                let ids: Vec<_> = streams.iter().map(|s| s.user_id).collect();
                let users: HashMap<_, _> = match twitch.get_users(&ids).await {
                    Ok(users) => users.into_iter().map(|u| (u.user_id, u)).collect(),
                    Err(why) => {
                        warn!("Error while retrieving twitch users: {}", why);
                        return;
                    }
                };

                let mut fmt_data = HashMap::new();
                fmt_data.insert(String::from("width"), String::from("360"));
                fmt_data.insert(String::from("height"), String::from("180"));

                // Put streams into a more suitable data type and process the thumbnail url
                let streams: HashMap<u64, TwitchStream> = streams
                    .into_iter()
                    .map(|mut stream| {
                        if let Ok(thumbnail) = strfmt(&stream.thumbnail_url, &fmt_data) {
                            stream.thumbnail_url = thumbnail;
                        }
                        (stream.user_id, stream)
                    })
                    .collect();

                // Process each tracking by notifying corresponding channels
                for track in stream_tracks {
                    if streams.contains_key(&track.user_id) {
                        let stream = streams.get(&track.user_id).unwrap();
                        let data =
                            TwitchNotifEmbed::new(stream, users.get(&stream.user_id).unwrap());
                        let embed = data.build(EmbedBuilder::new()).build();
                        match ctx.http.create_message(track.channel_id).embed(embed) {
                            Ok(msg_fut) => {
                                if let Err(why) = msg_fut.await {
                                    warn!("Error while sending twitch notif: {}", why);
                                }
                            }
                            Err(why) => warn!("Invalid embed for twitch notif: {}", why),
                        }
                    }
                }
                Some(now_online)
            }
        };
        if let Some(now_online) = now_online {
            online_streams = now_online;
        }
    }
}
