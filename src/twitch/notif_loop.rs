use super::*;
use crate::{
    embeds::{EmbedData, TwitchNotifEmbed},
    Context,
};

use hashbrown::{HashMap, HashSet};
use std::{collections::HashMap as StdHashMap, sync::Arc};
use strfmt::strfmt;
use tokio::time::{interval, Duration};
use twilight_http::{
    api_error::{ApiError, ErrorCode, GeneralApiError},
    Error as TwilightError,
};

#[cold]
pub async fn twitch_loop(ctx: Arc<Context>) {
    if cfg!(debug_assertions) {
        info!("Skip twitch tracking on debug");

        return;
    }

    // Formatting of the embed image
    let mut fmt_data = StdHashMap::new();
    fmt_data.insert(String::from("width"), String::from("360"));
    fmt_data.insert(String::from("height"), String::from("180"));

    let mut online_streams = HashSet::new();
    let mut interval = interval(Duration::from_secs(10 * 60));
    interval.tick().await;

    loop {
        interval.tick().await;

        // Get data about what needs to be tracked for which channel
        let user_ids = ctx.tracked_users();

        // Get stream data about all streams that need to be tracked
        let mut streams = match ctx.clients.twitch.get_streams(&user_ids).await {
            Ok(streams) => streams,
            Err(why) => {
                unwind_error!(warn, why, "Error while retrieving streams: {}");

                continue;
            }
        };

        // Filter streams whether they're live
        streams.retain(TwitchStream::is_live);
        let now_online: HashSet<_> = streams.iter().map(|stream| stream.user_id).collect();

        // If there was no activity change since last time, don't do anything
        if now_online == online_streams {
            continue;
        }

        // Filter streams whether its already known they're live
        streams.retain(|stream| !online_streams.contains(&stream.user_id));

        let ids: Vec<_> = streams.iter().map(|s| s.user_id).collect();
        let users: HashMap<_, _> = match ctx.clients.twitch.get_users(&ids).await {
            Ok(users) => users.into_iter().map(|u| (u.user_id, u)).collect(),
            Err(why) => {
                unwind_error!(warn, why, "Error while retrieving twitch users: {}");

                continue;
            }
        };

        // Put streams into a more suitable data type and process the thumbnail url
        let streams: Vec<(u64, TwitchStream)> = streams
            .into_iter()
            .map(|mut stream| {
                if let Ok(thumbnail) = strfmt(&stream.thumbnail_url, &fmt_data) {
                    stream.thumbnail_url = thumbnail;
                }

                (stream.user_id, stream)
            })
            .collect();

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
                match ctx.http.create_message(channel).embed(embed.clone()) {
                    Ok(msg_fut) => {
                        let result = msg_fut.await;

                        if let Err(TwilightError::Response { error, .. }) = result {
                            match error {
                                ApiError::General(GeneralApiError {
                                    code: ErrorCode::UnknownChannel,
                                    ..
                                }) => {
                                    if let Err(why) =
                                        ctx.psql().remove_channel_tracks(channel.0).await
                                    {
                                        unwind_error!(
                                            warn, why,
                                                "Could not remove stream tracks from unknown channel {}: {}",
                                                channel
                                            );
                                    } else {
                                        debug!(
                                            "Removed twitch tracking of unknown channel {}",
                                            channel
                                        );
                                    }
                                }
                                why => warn!(
                                    "Error from API while sending twitch notif (channel {}): {}",
                                    channel, why
                                ),
                            }
                        } else if let Err(why) = result {
                            unwind_error!(
                                warn,
                                why,
                                "Error while sending twitch notif (channel {}): {}",
                                channel
                            );
                        }
                    }
                    Err(why) => unwind_error!(warn, why, "Invalid embed for twitch notif: {}"),
                }
            }
        }

        online_streams = now_online;
    }
}
