use std::{borrow::Cow, collections::HashMap, num::NonZeroU64, slice};

use bathbot_model::rosu_v2::user::User;
use bathbot_psql::model::osu::{TrackedOsuUserKey, TrackedOsuUserValue};
use bathbot_util::{constants::UNKNOWN_CHANNEL, EmbedBuilder, IntHasher};
use eyre::Report;
use rosu_v2::{
    prelude::{OsuError, Score},
    OsuResult,
};
use time::OffsetDateTime;
use twilight_http::{
    api_error::{ApiError, GeneralApiError},
    error::ErrorType as TwilightErrorType,
};
use twilight_model::id::Id;

use crate::{
    embeds::{EmbedData, TrackNotificationEmbed},
    manager::{
        redis::{osu::UserArgs, RedisData},
        OsuMap,
    },
    Context,
};

#[cold]
pub async fn osu_tracking_loop() {
    let osu = Context::osu();
    let tracking = Context::tracking();

    loop {
        if let Some((key, amount)) = tracking.pop().await {
            let TrackedOsuUserKey { user_id, mode } = key;

            let scores_fut = osu
                .user_scores(user_id)
                .best()
                .mode(mode)
                .limit(amount as usize);

            match scores_fut.await {
                Ok(scores) => {
                    // * Note: If scores are empty, (user_id, mode) will not be reset into the
                    //   tracking queue
                    if !scores.is_empty() {
                        process_osu_tracking(&scores, None).await
                    }
                }
                Err(OsuError::NotFound) => {
                    warn!(
                        user_id,
                        ?mode,
                        "Got 404 while retrieving scores, don't reset entry",
                    );

                    if let Err(err) = tracking.remove_user_all(user_id).await {
                        warn!(?err, "Failed to remove unknown user from tracking");
                    }
                }
                Err(err) => {
                    warn!(
                        user_id,
                        ?mode,
                        ?err,
                        "osu!api issue while retrieving user for tracking"
                    );

                    tracking.reset(key).await;
                }
            }
        }
    }
}

pub async fn process_osu_tracking(scores: &[Score], user: Option<&RedisData<User>>) {
    let tracking = Context::tracking();

    // Make sure scores is not empty
    let (key, new_last) = match scores.iter().max_by_key(|s| s.ended_at) {
        Some(score) => {
            let key = TrackedOsuUserKey {
                user_id: score.user_id,
                mode: score.mode,
            };

            (key, score.ended_at)
        }
        None => return,
    };

    // Make sure the user is being tracked in general
    let (channels, last) = match tracking.get_tracked(key).await {
        Some(TrackedOsuUserValue {
            channels,
            last_update,
        }) => (channels, last_update),
        None => return,
    };

    // Make sure the user is being tracked in any channel
    let max = match channels.values().max() {
        Some(max) => *max,
        None => return,
    };

    // If new top score, update the date
    if new_last > last {
        let update_fut = tracking.update_last_date(key, new_last);

        if let Err(err) = update_fut.await {
            warn!(?err, "Failed to update tracking date for user");
        }
    }

    tracking.reset(key).await;

    let mut user = TrackUser::new(key, user);

    // Process scores
    match score_loop(&mut user, max, last, scores, &channels).await {
        Ok(_) => {}
        Err(OsuError::NotFound) => {
            if let Err(err) = tracking.remove_user_all(key.user_id).await {
                warn!(?err, "Failed to remove unknown user from tracking");
            }
        }
        Err(err) => {
            warn!(?err, "osu!api error while tracking");
            Context::tracking().reset(key).await;
        }
    }
}

async fn score_loop(
    user: &mut TrackUser<'_>,
    max: u8,
    last: OffsetDateTime,
    scores: &[Score],
    channels: &HashMap<NonZeroU64, u8, IntHasher>,
) -> OsuResult<()> {
    for (idx, score) in (1..).zip(scores.iter()).take(max as usize) {
        // Skip if its an older score
        if score.ended_at <= last {
            continue;
        }

        let checksum = score.map.as_ref().and_then(|map| map.checksum.as_deref());

        let map = match Context::osu_map().map(score.map_id, checksum).await {
            Ok(map) => map,
            Err(err) => {
                warn!("{:?}", Report::new(err));

                continue;
            }
        };

        let embed = user.embed(score, &map, idx).await?.build();
        let http = Context::http();
        let tracking = Context::tracking();

        // Send the embed to each tracking channel
        for (&channel, &limit) in channels.iter() {
            if idx > limit {
                continue;
            }

            let channel = Id::new(channel.get());
            let embeds = slice::from_ref(&embed);

            // Try to build and send the message
            match http.create_message(channel).embeds(embeds) {
                Ok(msg_fut) => {
                    if let Err(err) = msg_fut.await {
                        if let TwilightErrorType::Response { error, .. } = err.kind() {
                            if let ApiError::General(GeneralApiError {
                                code: UNKNOWN_CHANNEL,
                                ..
                            }) = error
                            {
                                let remove_fut = tracking.remove_channel(channel, None);

                                if let Err(err) = remove_fut.await {
                                    warn!(
                                        ?channel,
                                        ?err,
                                        "Failed to remove osu tracks from unknown channel"
                                    );
                                }
                            } else {
                                warn!(%channel, ?error, "Error from API while sending osu notif")
                            }
                        } else {
                            warn!(%channel, ?err, "Error while sending osu notif");
                        }
                    }
                }
                Err(err) => {
                    warn!(?err, "Invalid embed for osu!tracking notification");
                }
            }
        }
    }

    Ok(())
}

struct TrackUser<'u> {
    key: TrackedOsuUserKey,
    user: Option<Cow<'u, RedisData<User>>>,
}

impl<'u> TrackUser<'u> {
    #[inline]
    fn new(key: TrackedOsuUserKey, user: Option<&'u RedisData<User>>) -> Self {
        Self {
            key,
            user: user.map(Cow::Borrowed),
        }
    }

    async fn embed(&mut self, score: &Score, map: &OsuMap, idx: u8) -> OsuResult<EmbedBuilder> {
        let data = if let Some(user) = self.user.as_deref() {
            TrackNotificationEmbed::new(user, score, map, idx).await
        } else {
            let TrackedOsuUserKey { user_id, mode } = self.key;
            let args = UserArgs::user_id(user_id).mode(mode);
            let user = Context::redis().osu_user(args).await?;
            let user = self.user.get_or_insert(Cow::Owned(user));

            TrackNotificationEmbed::new(user.as_ref(), score, map, idx).await
        };

        Ok(data.build())
    }
}
