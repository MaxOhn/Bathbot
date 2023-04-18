use std::{borrow::Cow, collections::HashMap, num::NonZeroU64, slice, sync::Arc};

use bathbot_model::rosu_v2::user::User;
use bathbot_psql::model::osu::{TrackedOsuUserKey, TrackedOsuUserValue};
use bathbot_util::{constants::UNKNOWN_CHANNEL, IntHasher};
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
use twilight_model::{channel::message::embed::Embed, id::Id};

use crate::{
    embeds::{EmbedData, TrackNotificationEmbed},
    manager::{
        redis::{osu::UserArgs, RedisData},
        OsuMap,
    },
    Context,
};

#[cold]
pub async fn osu_tracking_loop(ctx: Arc<Context>) {
    loop {
        if let Some((key, amount)) = ctx.tracking().pop().await {
            let TrackedOsuUserKey { user_id, mode } = key;

            let scores_fut = ctx
                .osu()
                .user_scores(user_id)
                .best()
                .mode(mode)
                .limit(amount as usize);

            match scores_fut.await {
                Ok(scores) => {
                    // * Note: If scores are empty, (user_id, mode) will not be reset into the tracking queue
                    if !scores.is_empty() {
                        process_osu_tracking(&ctx, &scores, None).await
                    }
                }
                Err(OsuError::NotFound) => {
                    warn!(
                        "got 404 while retrieving scores for ({user_id},{mode}), don't reset entry",
                    );

                    if let Err(err) = ctx
                        .tracking()
                        .remove_user_all(user_id, ctx.osu_tracking())
                        .await
                    {
                        let wrap = "Failed to remove unknown user from tracking";
                        warn!("{:?}", err.wrap_err(wrap));
                    }
                }
                Err(err) => {
                    let wrap = format!(
                        "osu!api issue while retrieving user ({user_id},{mode}) for tracking",
                    );
                    let err = Report::new(err).wrap_err(wrap);
                    warn!("{err:?}");
                    ctx.tracking().reset(key).await;
                }
            }
        }
    }
}

pub async fn process_osu_tracking(ctx: &Context, scores: &[Score], user: Option<&RedisData<User>>) {
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
    let (channels, last) = match ctx.tracking().get_tracked(key).await {
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
        let update_fut = ctx
            .tracking()
            .update_last_date(key, new_last, ctx.osu_tracking());

        if let Err(err) = update_fut.await {
            let wrap = "failed to update tracking date for user";
            warn!("{:?}", err.wrap_err(wrap));
        }
    }

    ctx.tracking().reset(key).await;

    let mut user = TrackUser::new(key, user);

    // Process scores
    match score_loop(ctx, &mut user, max, last, scores, &channels).await {
        Ok(_) => {}
        Err(OsuError::NotFound) => {
            if let Err(err) = ctx
                .tracking()
                .remove_user_all(key.user_id, ctx.osu_tracking())
                .await
            {
                let wrap = "failed to remove unknow user from tracking";
                warn!("{:?}", err.wrap_err(wrap));
            }
        }
        Err(err) => {
            let err = Report::new(err).wrap_err("osu!api error while tracking");
            warn!("{err:?}");
            ctx.tracking().reset(key).await;
        }
    }
}

async fn score_loop(
    ctx: &Context,
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

        let map = match ctx.osu_map().map(score.map_id, checksum).await {
            Ok(map) => map,
            Err(err) => {
                warn!("{:?}", Report::new(err));

                continue;
            }
        };

        let embed = user.embed(ctx, score, &map, idx).await?;

        // Send the embed to each tracking channel
        for (&channel, &limit) in channels.iter() {
            if idx > limit {
                continue;
            }

            let channel = Id::new(channel.get());
            let embeds = slice::from_ref(&embed);

            // Try to build and send the message
            match ctx.http.create_message(channel).embeds(embeds) {
                Ok(msg_fut) => {
                    if let Err(err) = msg_fut.await {
                        if let TwilightErrorType::Response { error, .. } = err.kind() {
                            if let ApiError::General(GeneralApiError {
                                code: UNKNOWN_CHANNEL,
                                ..
                            }) = error
                            {
                                let remove_fut = ctx.tracking().remove_channel(
                                    channel,
                                    None,
                                    ctx.osu_tracking(),
                                );

                                if let Err(err) = remove_fut.await {
                                    let wrap = format!(
                                        "failed to remove osu tracks from unknown channel {channel}",
                                    );

                                    warn!("{:?}", err.wrap_err(wrap));
                                }
                            } else {
                                warn!(
                                    "Error from API while sending osu notif (channel {channel}): {error}",
                                )
                            }
                        } else {
                            let wrap = format!("error while sending osu notif (channel {channel})");
                            let err = Report::new(err).wrap_err(wrap);
                            warn!("{err:?}");
                        }
                    }
                }
                Err(err) => {
                    let err =
                        Report::new(err).wrap_err("invalid embed for osu!tracking notification");
                    warn!("{err:?}");
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

    async fn embed(
        &mut self,
        ctx: &Context,
        score: &Score,
        map: &OsuMap,
        idx: u8,
    ) -> OsuResult<Embed> {
        let data = if let Some(user) = self.user.as_deref() {
            TrackNotificationEmbed::new(user, score, map, idx, ctx).await
        } else {
            let TrackedOsuUserKey { user_id, mode } = self.key;
            let args = UserArgs::user_id(user_id).mode(mode);
            let user = ctx.redis().osu_user(args).await?;
            let user = self.user.get_or_insert(Cow::Owned(user));

            TrackNotificationEmbed::new(user.as_ref(), score, map, idx, ctx).await
        };

        Ok(data.build())
    }
}
