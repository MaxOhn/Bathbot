use std::{borrow::Cow, sync::Arc};

use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::{
    prelude::{GameMode, OsuError, Score, User},
    OsuResult,
};
use time::OffsetDateTime;
use twilight_http::{
    api_error::{ApiError, GeneralApiError},
    error::ErrorType as TwilightErrorType,
};
use twilight_model::{
    channel::embed::Embed,
    id::{marker::ChannelMarker, Id},
};

use crate::{
    commands::osu::prepare_score,
    embeds::{EmbedData, TrackNotificationEmbed},
    util::{constants::UNKNOWN_CHANNEL, hasher::IntHasher},
    Context,
};

use super::osu_queue::TrackingEntry;

#[cold]
pub async fn osu_tracking_loop(ctx: Arc<Context>) {
    loop {
        if let Some((entry, amount)) = ctx.tracking().pop().await {
            let TrackingEntry { user_id, mode } = entry;

            let scores_fut = ctx
                .osu()
                .user_scores(user_id)
                .best()
                .mode(mode)
                .limit(amount);

            match scores_fut.await {
                Ok(mut scores) => {
                    // * Note: If scores are empty, (user_id, mode) will not be reset into the tracking queue
                    if !scores.is_empty() {
                        process_osu_tracking(&ctx, &mut scores, None).await
                    }
                }
                Err(OsuError::NotFound) => {
                    warn!(
                        "got 404 while retrieving scores for ({user_id},{mode}), don't reset entry",
                    );

                    if let Err(err) = ctx.tracking().remove_user_all(user_id, ctx.psql()).await {
                        let wrap = "Failed to remove unknown user from tracking";
                        warn!("{:?}", err.wrap_err(wrap));
                    }
                }
                Err(err) => {
                    let wrap = format!(
                        "osu!api issue while retrieving user ({user_id},{mode}) for tracking",
                    );
                    let report = Report::new(err).wrap_err(wrap);
                    warn!("{report:?}");
                    ctx.tracking().reset(user_id, mode).await;
                }
            }
        }
    }
}

pub async fn process_osu_tracking(ctx: &Context, scores: &mut [Score], user: Option<&User>) {
    // Make sure scores is not empty
    let (user_id, mode, new_last) = match scores.iter().max_by_key(|s| s.ended_at) {
        Some(score) => (score.user_id, score.mode, score.ended_at),
        None => return,
    };

    // Make sure the user is being tracked in general
    let (last, channels) = match ctx.tracking().get_tracked(user_id, mode).await {
        Some(tuple) => tuple,
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
            .update_last_date(user_id, mode, new_last, ctx.psql());

        if let Err(err) = update_fut.await {
            let wrap = format!("Failed to update tracking date for user ({user_id},{mode})");
            warn!("{:?}", err.wrap_err(wrap));
        }
    }

    ctx.tracking().reset(user_id, mode).await;

    let mut user = TrackUser::new(user_id, mode, user);

    // Process scores
    match score_loop(ctx, &mut user, max, last, scores, &channels).await {
        Ok(_) => {}
        Err(OsuError::NotFound) => {
            if let Err(err) = ctx.tracking().remove_user_all(user_id, ctx.psql()).await {
                let wrap = "Failed to remove unknow user from tracking";
                warn!("{:?}", err.wrap_err(wrap));
            }
        }
        Err(err) => {
            let report = Report::new(err).wrap_err("osu!api error while tracking");
            warn!("{report:?}");
            ctx.tracking().reset(user_id, mode).await;
        }
    }
}

async fn score_loop(
    ctx: &Context,
    user: &mut TrackUser<'_>,
    max: usize,
    last: OffsetDateTime,
    scores: &mut [Score],
    channels: &HashMap<Id<ChannelMarker>, usize, IntHasher>,
) -> OsuResult<()> {
    for (idx, score) in (1..).zip(scores.iter_mut()).take(max) {
        // Skip if its an older score
        if score.ended_at <= last {
            continue;
        }

        let requires_combo = score.map.as_ref().map_or(false, |m| {
            matches!(m.mode, GameMode::Osu | GameMode::Catch) && m.max_combo.is_none()
        });

        if requires_combo {
            if let Err(err) = prepare_score(ctx, score).await {
                let report = Report::new(err).wrap_err("failed to fill in max combo for tracking");
                warn!("{report:?}");

                continue;
            }
        }

        // Send the embed to each tracking channel
        for (&channel, &limit) in channels.iter() {
            if idx > limit {
                continue;
            }

            let embed = user.embed(ctx, score, idx).await?;

            // Try to build and send the message
            match ctx.http.create_message(channel).embeds(&[embed]) {
                Ok(msg_fut) => {
                    if let Err(err) = msg_fut.exec().await {
                        if let TwilightErrorType::Response { error, .. } = err.kind() {
                            if let ApiError::General(GeneralApiError {
                                code: UNKNOWN_CHANNEL,
                                ..
                            }) = error
                            {
                                let remove_fut =
                                    ctx.tracking().remove_channel(channel, None, ctx.psql());

                                if let Err(err) = remove_fut.await {
                                    let wrap = format!(
                                        "Failed to remove osu tracks from unknown channel {channel}",
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
                            let report = Report::new(err).wrap_err(wrap);
                            warn!("{report:?}");
                        }
                    }
                }
                Err(err) => {
                    let report =
                        Report::new(err).wrap_err("invalid embed for osu!tracking notification");
                    warn!("{report:?}");
                }
            }
        }
    }

    Ok(())
}

struct TrackUser<'u> {
    user_id: u32,
    mode: GameMode,
    user: Option<Cow<'u, User>>,
}

impl<'u> TrackUser<'u> {
    #[inline]
    fn new(user_id: u32, mode: GameMode, user: Option<&'u User>) -> Self {
        Self {
            user_id,
            mode,
            user: user.map(Cow::Borrowed),
        }
    }

    async fn embed(&mut self, ctx: &Context, score: &Score, idx: usize) -> OsuResult<Embed> {
        let data = if let Some(user) = self.user.as_deref() {
            TrackNotificationEmbed::new(user, score, idx, ctx).await
        } else {
            let user = ctx.osu().user(self.user_id).mode(self.mode).await?;
            let user = self.user.get_or_insert(Cow::Owned(user));

            TrackNotificationEmbed::new(user.as_ref(), score, idx, ctx).await
        };

        Ok(data.build())
    }
}
