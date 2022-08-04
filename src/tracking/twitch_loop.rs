use std::{fmt::Write, sync::Arc};

use eyre::Report;
use hashbrown::{HashMap, HashSet};
use rand::Rng;
use tokio::time::{interval, Duration};
use twilight_http::{
    api_error::{ApiError, GeneralApiError},
    error::ErrorType,
};
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{
    custom_client::TwitchStream,
    embeds::{EmbedData, TwitchNotifEmbed},
    util::{constants::UNKNOWN_CHANNEL, hasher::SimpleBuildHasher},
    Context,
};

#[cold]
pub async fn twitch_tracking_loop(ctx: Arc<Context>) {
    if cfg!(debug_assertions) {
        info!("Skip twitch tracking on debug");

        return;
    }

    let mut online_streams = HashSet::with_hasher(SimpleBuildHasher);
    let mut interval = interval(Duration::from_secs(10 * 60));
    interval.tick().await;

    loop {
        interval.tick().await;

        // Get data about what needs to be tracked for which channel
        let user_ids = ctx.tracked_users();

        // Get stream data about all streams that need to be tracked
        let mut streams = match ctx.client().get_twitch_streams(&user_ids).await {
            Ok(streams) => streams,
            Err(err) => {
                let report = Report::new(err);
                warn!("{:?}", report.wrap_err("error while retrieving streams"));

                continue;
            }
        };

        // Filter streams whether they're live
        streams.retain(TwitchStream::is_live);
        let now_online: HashSet<_, SimpleBuildHasher> =
            streams.iter().map(|stream| stream.user_id).collect();

        // If there was no activity change since last time, don't do anything
        if now_online == online_streams {
            continue;
        }

        // Filter streams whether its already known they're live
        streams.retain(|stream| !online_streams.contains(&stream.user_id));

        // Nothing to do if streams is empty
        // (i.e. the change was that streamers went offline)
        if streams.is_empty() {
            online_streams = now_online;

            continue;
        }

        let ids: Vec<_> = streams.iter().map(|s| s.user_id).collect();

        let users: HashMap<_, _> = match ctx.client().get_twitch_users(&ids).await {
            Ok(users) => users.into_iter().map(|u| (u.user_id, u)).collect(),
            Err(err) => {
                let report = Report::new(err).wrap_err("error while retrieving twitch users");
                warn!("{report:?}");

                continue;
            }
        };

        // Generate random width and height to avoid discord caching the thumbnail url
        let (width, height) = {
            let mut rng = rand::thread_rng();

            let width: u32 = rng.gen_range(350..=370);
            let height: u32 = rng.gen_range(175..=185);

            (width, height)
        };

        // Process each stream by notifying all corresponding channels
        for mut stream in streams {
            let channels = match ctx.tracked_channels_for(stream.user_id) {
                Some(channels) => channels,
                None => continue,
            };

            // Adjust streams' thumbnail url
            let url_len = stream.thumbnail_url.len();
            stream.thumbnail_url.truncate(url_len - 20); // cut off "{width}x{height}.jpg"
            let _ = write!(stream.thumbnail_url, "{width}x{height}.jpg");

            let data = TwitchNotifEmbed::new(&stream, &users[&stream.user_id]);

            for channel in channels {
                send_notif(&ctx, &data, channel).await;
            }
        }

        online_streams = now_online;
    }
}

async fn send_notif(ctx: &Context, data: &TwitchNotifEmbed, channel: Id<ChannelMarker>) {
    let embed = data.to_owned().build();

    match ctx.http.create_message(channel).embeds(&[embed]) {
        Ok(msg_fut) => {
            if let Err(err) = msg_fut.exec().await {
                if let ErrorType::Response { error, .. } = err.kind() {
                    match error {
                        ApiError::General(GeneralApiError {
                            code: UNKNOWN_CHANNEL,
                            ..
                        }) => {
                            if let Err(err) = ctx.psql().remove_channel_tracks(channel.get()).await
                            {
                                let wrap = format!(
                                    "could not remove stream tracks from unknown channel {channel}"
                                );

                                let report = Report::new(err).wrap_err(wrap);
                                warn!("{report:?}");
                            } else {
                                debug!("Removed twitch tracking of unknown channel {channel}");
                            }
                        }
                        err => warn!(
                            "Error from API while sending twitch notif (channel {channel}): {err}"
                        ),
                    }
                } else {
                    let wrap = format!("error while sending twitch notif (channel {channel})");
                    let report = Report::new(err).wrap_err(wrap);
                    warn!("{report:?}");
                }
            }
        }
        Err(err) => {
            let report = Report::new(err).wrap_err("invalid embed for twitch notif");
            warn!("{report:?}");
        }
    }
}
