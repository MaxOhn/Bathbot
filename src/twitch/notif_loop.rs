use super::*;
use crate::{
    embeds::{EmbedData, TwitchNotifEmbed},
    Context,
};

use rayon::prelude::*;
use reqwest::StatusCode;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use strfmt::strfmt;
use tokio::time;
use twilight::http::Error as TwilightError;

pub async fn twitch_loop(ctx: Arc<Context>) {
    // Formatting of the embed image
    let mut fmt_data = HashMap::new();
    fmt_data.insert(String::from("width"), String::from("360"));
    fmt_data.insert(String::from("height"), String::from("180"));

    let mut online_streams = HashSet::new();
    let mut interval = time::interval(time::Duration::from_secs(10 * 60));
    interval.tick().await;
    loop {
        interval.tick().await;
        let now_online = {
            // Get data about what needs to be tracked for which channel
            let user_ids = ctx.tracked_users();

            // Get stream data about all streams that need to be tracked
            let mut streams = match ctx.clients.twitch.get_streams(&user_ids).await {
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
                let users: HashMap<_, _> = match ctx.clients.twitch.get_users(&ids).await {
                    Ok(users) => users.into_iter().map(|u| (u.user_id, u)).collect(),
                    Err(why) => {
                        warn!("Error while retrieving twitch users: {}", why);
                        return;
                    }
                };

                // Put streams into a more suitable data type and process the thumbnail url
                let streams: Vec<(u64, TwitchStream)> = streams
                    .into_par_iter()
                    .map(|mut stream| {
                        if let Ok(thumbnail) = strfmt(&stream.thumbnail_url, &fmt_data) {
                            stream.thumbnail_url = thumbnail;
                        }
                        (stream.user_id, stream)
                    })
                    .collect();

                //

                // Process each stream by notifying all corresponding channels
                for (user, stream) in streams {
                    let channels = match ctx.tracked_channels_for(user) {
                        Some(channels) => channels,
                        None => continue,
                    };
                    let data = TwitchNotifEmbed::new(&stream, users.get(&stream.user_id).unwrap());
                    let embed = match data.build().build() {
                        Ok(embed) => embed,
                        Err(why) => {
                            error!("Error while creating twitch notif embed: {}", why);
                            continue;
                        }
                    };
                    for channel in channels {
                        let msg_fut = ctx.http.create_message(channel);
                        match msg_fut.embed(embed.clone()) {
                            Ok(msg_fut) => {
                                match msg_fut.await {
                                    Err(TwilightError::Response {
                                        status: StatusCode::FORBIDDEN,
                                        ..
                                    }) => {
                                        // If not in debug mode + on an arm system e.g. raspberry pi
                                        if cfg!(any(debug_assertions, not(target_arch = "arm"))) {
                                            continue;
                                        }
                                        if let Err(why) =
                                            ctx.psql().remove_channel_tracks(channel.0).await
                                        {
                                            warn!(
                                                "No permission to send twitch notif in channel \
                                                {} but could not remove channel tracks: {}",
                                                channel, why
                                            );
                                        } else {
                                            debug!(
                                                "Removed twitch tracking in channel {} \
                                                because of no SEND_PERMISSION",
                                                channel
                                            );
                                        }
                                    }
                                    Err(why) => warn!("Error while sending twitch notif: {}", why),
                                    _ => {}
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
