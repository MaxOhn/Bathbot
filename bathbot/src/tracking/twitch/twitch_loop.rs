use std::{fmt::Write, sync::Arc};

use bathbot_model::TwitchUser;
use bathbot_util::{
    constants::{TWITCH_BASE, UNKNOWN_CHANNEL},
    AuthorBuilder, EmbedBuilder, IntHasher,
};
use hashbrown::{HashMap, HashSet};
use rand::Rng;
use tokio::time::{interval, Duration};
use twilight_http::{
    api_error::{ApiError, GeneralApiError},
    error::ErrorType,
};
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::Context;

#[cold]
pub async fn twitch_tracking_loop(ctx: Arc<Context>) {
    let mut online_streams = HashSet::with_hasher(IntHasher);
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
                warn!(?err, "Failed to retrieve streams");

                continue;
            }
        };

        // Filter streams whether they're live
        {
            let guard = ctx.online_twitch_streams().guard();

            streams.retain(|stream| {
                if stream.live {
                    ctx.online_twitch_streams().set_online(stream, &guard);
                } else {
                    ctx.online_twitch_streams().set_offline(stream, &guard);
                }

                stream.live
            });
        }

        let now_online: HashSet<_, IntHasher> =
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

        let users: HashMap<_, _, IntHasher> = match ctx.client().get_twitch_users(&ids).await {
            Ok(users) => users
                .into_iter()
                .map(|u| (u.user_id, TwitchUserCompact::from(u)))
                .collect(),
            Err(err) => {
                warn!(?err, "Failed to retrieve twitch users");

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
            let Some(channels) = ctx.tracked_channels_for(stream.user_id) else { continue };

            // Adjust streams' thumbnail url
            let url_len = stream.thumbnail_url.len();
            stream.thumbnail_url.truncate(url_len - 20); // cut off "{width}x{height}.jpg"
            let _ = write!(stream.thumbnail_url, "{width}x{height}.jpg");

            let user = &users[&stream.user_id];

            let embed = EmbedBuilder::new()
                .author(AuthorBuilder::new("Now live on twitch:"))
                .description(stream.title.as_ref())
                .image(&stream.thumbnail_url)
                .thumbnail(user.image_url.as_ref())
                .title(stream.username.as_ref())
                .url(format!("{TWITCH_BASE}{}", user.display_name));

            let mut channels = channels.into_iter();
            let last = channels.next_back();

            for channel in channels {
                send_notif(&ctx, embed.clone(), channel).await;
            }

            // doing last one separately so we don't clone embed
            if let Some(channel) = last {
                send_notif(&ctx, embed, channel).await;
            }
        }

        online_streams = now_online;
    }
}

async fn send_notif(ctx: &Context, embed: EmbedBuilder, channel: Id<ChannelMarker>) {
    let embed = embed.build();

    match ctx.http.create_message(channel).embeds(&[embed]) {
        Ok(msg_fut) => {
            if let Err(err) = msg_fut.await {
                if let ErrorType::Response { error, .. } = err.kind() {
                    match error {
                        ApiError::General(GeneralApiError {
                            code: UNKNOWN_CHANNEL,
                            ..
                        }) => {
                            if let Err(err) = ctx.twitch().untrack_all(channel).await {
                                warn!(
                                    %channel,
                                    ?err,
                                    "Failed to remove stream tracks from unknown channel"
                                );
                            } else {
                                debug!("Removed twitch tracking of unknown channel {channel}");
                            }
                        }
                        err => warn!(
                            %channel,
                            ?err,
                            "Error from API while sending twitch notif"
                        ),
                    }
                } else {
                    warn!(
                        %channel,
                        ?err,
                        "Error while sending twitch notif"
                    );
                }
            }
        }
        Err(err) => {
            warn!(?err, "Invalid embed for twitch notif");
        }
    }
}

struct TwitchUserCompact {
    display_name: Box<str>,
    image_url: Box<str>,
}

impl From<TwitchUser> for TwitchUserCompact {
    fn from(user: TwitchUser) -> Self {
        Self {
            display_name: user.display_name,
            image_url: user.image_url,
        }
    }
}
