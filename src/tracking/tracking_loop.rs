use crate::{
    embeds::{EmbedData, TrackNotificationEmbed},
    unwind_error, Context,
};

use futures::future::{join_all, FutureExt};
use rosu::model::{Beatmap, GameMode, Score, User};
use std::{collections::HashMap, sync::Arc};
use tokio::time;
use twilight_http::{
    api_error::{ApiError, ErrorCode, GeneralApiError},
    Error as TwilightError,
};

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
        let score_futs = tracked.iter().map(|&(user_id, mode)| {
            ctx.osu()
                .top_scores(user_id)
                .mode(mode)
                .limit(100)
                .map(move |result| (user_id, mode, result))
        });

        // Iterate over the request responses
        let mut maps: HashMap<u32, Beatmap> = HashMap::new();

        for (user_id, mode, result) in join_all(score_futs).await {
            match result {
                Ok(scores) => {
                    // Note: If scores are empty, (user_id, mode) will not be reset into the tracking queue
                    if !scores.is_empty() {
                        process_tracking(&ctx, mode, &scores, None, &mut maps).await
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

pub async fn process_tracking(
    ctx: &Context,
    mode: GameMode,
    scores: &[Score],
    user: Option<&User>,
    maps: &mut HashMap<u32, Beatmap>,
) {
    let id_option = scores
        .first()
        .map(|s| s.user_id)
        .or_else(|| user.map(|u| u.user_id));

    let user_id = match id_option {
        Some(id) => id,
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

    let new_last = match scores.iter().map(|s| s.date).max() {
        Some(new_last) => new_last,
        None => return,
    };

    debug!(
        "[Tracking] ({},{}): last {} | curr {}",
        user_id, mode, last, new_last
    );

    let mut user_value = None; // will be set if user is None but there is new top score

    for (idx, score) in scores.iter().enumerate().take(max) {
        // Skip if its an older score
        if score.date <= last {
            continue;
        }

        debug!(
            "[New top score] ({},{}): new {} | old {}",
            user_id, mode, score.date, last
        );

        // Prepare beatmap
        let map_id = match score.beatmap_id {
            Some(id) => id,
            None => {
                warn!("No beatmap_id for ({},{})'s score", user_id, mode);
                continue;
            }
        };

        if !maps.contains_key(&map_id) {
            match ctx.psql().get_beatmap(map_id).await {
                Ok(map) => maps.insert(map_id, map),
                Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
                    Ok(Some(map)) => maps.insert(map_id, map),
                    Ok(None) => {
                        warn!("Beatmap id {} was not found for tracking", map_id);

                        continue;
                    }
                    Err(why) => {
                        unwind_error!(
                            warn,
                            why,
                            "Error while retrieving tracking map id {}: {}",
                            map_id
                        );

                        continue;
                    }
                },
            };
        }

        let map = maps.get(&map_id).unwrap();

        // Prepare user
        let user = match (user, user_value.as_ref()) {
            (Some(user), _) => user,
            (None, Some(user)) => user,
            (None, None) => match ctx.osu().user(user_id).mode(mode).await {
                Ok(Some(user)) => {
                    user_value = Some(user);
                    user_value.as_ref().unwrap()
                }
                Ok(None) => {
                    warn!("Empty result while retrieving tracking user {}", user_id);

                    continue;
                }
                Err(why) => {
                    unwind_error!(
                        warn,
                        why,
                        "Error while retrieving tracking user {}: {}",
                        user_id
                    );

                    continue;
                }
            },
        };

        // Build embed
        let data = TrackNotificationEmbed::new(user, score, map, idx + 1).await;

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
                            let result = ctx
                                .tracking()
                                .remove_channel(channel, None, ctx.psql())
                                .await;

                            if let Err(why) = result {
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
