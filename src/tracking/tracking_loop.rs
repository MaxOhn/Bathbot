use crate::{
    commands::osu::prepare_score,
    embeds::{EmbedData, TrackNotificationEmbed},
    Context,
};

use chrono::{DateTime, Utc};
use futures::{
    future::FutureExt,
    stream::{FuturesUnordered, StreamExt},
};
use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, Score};
use std::sync::Arc;
use tokio::time;
use twilight_http::{
    api_error::{ApiError, ErrorCode, GeneralApiError},
    Error as TwilightError,
};
use twilight_model::id::ChannelId;

#[cold]
pub async fn tracking_loop(ctx: Arc<Context>) {
    if cfg!(debug_assertions) {
        info!("Skip osu! tracking on debug");

        return;
    }

    let delay = time::Duration::from_secs(60);

    loop {
        // Get all users that should be tracked in this iteration
        let tracked = match ctx.tracking().pop().await {
            Some(tracked) => tracked,
            None => {
                time::sleep(delay).await;

                continue;
            }
        };

        // Build top score requests for each
        let mut scores_futs = tracked
            .iter()
            .map(|&(user_id, mode)| {
                ctx.osu()
                    .user_scores(user_id)
                    .best()
                    .mode(mode)
                    .limit(50)
                    .map(move |result| (user_id, mode, result))
            })
            .collect::<FuturesUnordered<_>>();

        // Iterate over the request responses
        while let Some((user_id, mode, result)) = scores_futs.next().await {
            match result {
                Ok(mut scores) => {
                    // Note: If scores are empty, (user_id, mode) will not be reset into the tracking queue
                    if !scores.is_empty() {
                        process_tracking(&ctx, mode, &mut scores).await
                    }
                }
                Err(why) => {
                    unwind_error!(
                        warn,
                        why,
                        "API issue while retrieving user ({},{}) for tracking: {}",
                        user_id,
                        mode
                    );

                    ctx.tracking().reset(user_id, mode).await;
                }
            }
        }
    }
}

pub async fn process_tracking(ctx: &Context, mode: GameMode, scores: &mut [Score]) {
    let user_id = match scores.first().map(|s| s.user_id) {
        Some(user_id) => user_id,
        None => return,
    };

    let (last, channels) = match ctx.tracking().get_tracked(user_id, mode) {
        Some(tuple) => tuple,
        None => return,
    };

    let max = match channels.values().max() {
        Some(max) => *max,
        None => return,
    };

    let mut new_last = match scores.iter().map(|s| s.created_at).max() {
        Some(new_last) => new_last,
        None => return,
    };

    debug!(
        "[Tracking] ({},{}): last {} | curr {}",
        user_id, mode, last, new_last
    );

    // Process scores
    score_loop(ctx, user_id, mode, 0, max, last, scores, &channels).await;

    let count = scores.len();

    // If another load of scores is requires, request and process them
    if count < max {
        let scores_fut = ctx
            .osu()
            .user_scores(user_id)
            .offset(count)
            .limit(max - count)
            .mode(mode);

        match scores_fut.await {
            Ok(mut scores) => {
                if let Some(max) = scores.iter().map(|s| s.created_at).max() {
                    new_last = new_last.max(max);
                }

                score_loop(
                    ctx,
                    user_id,
                    mode,
                    count,
                    max - count,
                    last,
                    &mut scores,
                    &channels,
                )
                .await;
            }
            Err(why) => unwind_error!(
                warn,
                why,
                "Failed to request second load of scores for tracking: {}"
            ),
        }
    }

    // If new top score, update the date
    if new_last > last {
        debug!(
            "[Tracking] Updating for ({},{}): {} -> {}",
            user_id, mode, last, new_last
        );

        let update_fut = ctx
            .tracking()
            .update_last_date(user_id, mode, new_last, ctx.psql());

        if let Err(why) = update_fut.await {
            unwind_error!(
                warn,
                why,
                "Error while updating tracking date for user ({},{}): {}",
                user_id,
                mode
            );
        }
    }

    ctx.tracking().reset(user_id, mode).await;
    debug!("[Tracking] Reset ({},{})", user_id, mode);
}

#[allow(clippy::too_many_arguments)]
async fn score_loop(
    ctx: &Context,
    user_id: u32,
    mode: GameMode,
    start: usize,
    max: usize,
    last: DateTime<Utc>,
    scores: &mut [Score],
    channels: &HashMap<ChannelId, usize>,
) {
    for (mut idx, score) in scores.iter_mut().enumerate().take(max) {
        idx += start;

        // Skip if its an older score
        if score.created_at <= last {
            continue;
        }

        let requires_combo = score.map.as_ref().map_or(false, |m| {
            matches!(m.mode, GameMode::STD | GameMode::CTB) && m.max_combo.is_none()
        });

        if requires_combo {
            if let Err(why) = prepare_score(&ctx, score).await {
                unwind_error!(warn, why, "Failed to fill in max combo for tracking: {}");

                continue;
            }
        }

        debug!(
            "[New top score] ({},{}): new {} | old {}",
            user_id, mode, score.created_at, last
        );

        // Build embed
        let data = TrackNotificationEmbed::new(score, idx + 1).await;

        let embed = match data.build().build() {
            Ok(embed) => embed,
            Err(why) => {
                unwind_error!(
                    warn,
                    why,
                    "Error while creating tracking notification embed: {}"
                );

                continue;
            }
        };

        // Send the embed to each tracking channel
        for (&channel, &limit) in channels.iter() {
            if idx + 1 > limit {
                continue;
            }

            // Try to build and send the message
            match ctx.http.create_message(channel).embed(embed.clone()) {
                Ok(msg_fut) => {
                    let result = msg_fut.await;

                    if let Err(TwilightError::Response { error, .. }) = result {
                        if let ApiError::General(GeneralApiError {
                            code: ErrorCode::UnknownChannel,
                            ..
                        }) = error
                        {
                            let remove_fut =
                                ctx.tracking().remove_channel(channel, None, ctx.psql());

                            if let Err(why) = remove_fut.await {
                                unwind_error!(
                                    warn,
                                    why,
                                    "Could not remove osu tracks from unknown channel {}: {}",
                                    channel
                                );
                            } else {
                                debug!("Removed osu tracking of unknown channel {}", channel);
                            }
                        } else {
                            warn!(
                                "Error from API while sending osu notif (channel {}): {}",
                                channel, error
                            )
                        }
                    } else if let Err(why) = result {
                        unwind_error!(
                            warn,
                            why,
                            "Error while sending osu notif (channel {}): {}",
                            channel
                        );
                    }
                }
                Err(why) => {
                    unwind_error!(warn, why, "Invalid embed for osu!tracking notification: {}")
                }
            }
        }
    }
}
