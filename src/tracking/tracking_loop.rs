use std::sync::Arc;

use chrono::{DateTime, Utc};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::{
    prelude::{GameMode, OsuError, Score, User},
    OsuResult,
};
use tokio::sync::mpsc::Receiver;
use twilight_http::{
    api_error::{ApiError, ErrorCode, GeneralApiError},
    error::ErrorType as TwilightErrorType,
};
use twilight_model::{channel::embed::Embed, id::ChannelId};

use crate::{
    commands::osu::prepare_score,
    embeds::{EmbedData, TrackNotificationEmbed},
    Context,
};

use super::TrackingEntry;

#[cold]
pub async fn tracking_loop(ctx: Arc<Context>, mut rx: Receiver<TrackingEntry>) {
    if cfg!(debug_assertions) {
        info!("Skip osu! tracking on debug");

        return;
    }

    loop {
        ctx.tracking().pop().await;

        while let Some(TrackingEntry { user_id, mode }) = rx.recv().await {
            // TODO: Max limit of any channel(?)
            let scores_fut = ctx.osu().user_scores(user_id).best().mode(mode).limit(50);

            match scores_fut.await {
                Ok(mut scores) => {
                    // * Note: If scores are empty, (user_id, mode) will not be reset into the tracking queue
                    if !scores.is_empty() {
                        process_tracking(&ctx, mode, &mut scores, None).await
                    }
                }
                Err(OsuError::NotFound) => {
                    warn!(
                        "got 404 while retrieving scores for ({},{}), don't reset entry",
                        user_id, mode
                    );

                    if let Err(why) = ctx.tracking().remove_user_all(user_id, ctx.psql()).await {
                        let report = Report::new(why)
                            .wrap_err("failed to remove unknown user from tracking");
                        warn!("{:?}", report);
                    }
                }
                Err(why) => {
                    let wrap = format!(
                        "osu!api issue while retrieving user ({},{}) for tracking",
                        user_id, mode
                    );
                    let report = Report::new(why).wrap_err(wrap);
                    warn!("{:?}", report);
                    ctx.tracking().reset(user_id, mode);
                }
            }
        }
    }
}

pub async fn process_tracking(
    ctx: &Context,
    mode: GameMode,
    scores: &mut [Score],
    user: Option<&User>,
) {
    // Make sure scores is not empty
    let user_id = match scores.first().map(|s| s.user_id) {
        Some(user_id) => user_id,
        None => return,
    };

    // Make sure the user is being tracked in general
    let (last, channels) = match ctx.tracking().get_tracked(user_id, mode) {
        Some(tuple) => tuple,
        None => return,
    };

    // Make sure the user is being tracked in any channel
    let max = match channels.values().max() {
        Some(max) => *max,
        None => return,
    };

    let new_last = match scores.iter().map(|s| s.created_at).max() {
        Some(new_last) => new_last,
        None => return,
    };

    // If new top score, update the date
    if new_last > last {
        let update_fut = ctx
            .tracking()
            .update_last_date(user_id, mode, new_last, ctx.psql());

        if let Err(why) = update_fut.await {
            let wrap = format!(
                "error while updating tracking date for user ({},{})",
                user_id, mode
            );

            let report = Report::new(why).wrap_err(wrap);
            warn!("{:?}", report);
        }
    }

    ctx.tracking().reset(user_id, mode);

    let mut user = TrackUser::new(user_id, mode, user);

    // Process scores
    match score_loop(ctx, &mut user, max, last, scores, &channels).await {
        Ok(_) => {}
        Err(OsuError::NotFound) => {
            if let Err(err) = ctx.tracking().remove_user_all(user_id, ctx.psql()).await {
                let report =
                    Report::new(err).wrap_err("failed to remove unknow user from tracking");
                warn!("{:?}", report);
            }
        }
        Err(err) => {
            let report = Report::new(err).wrap_err("osu!api error while tracking");
            warn!("{:?}", report);
            ctx.tracking().reset(user_id, mode);
        }
    }
}

async fn score_loop(
    ctx: &Context,
    user: &mut TrackUser<'_>,
    max: usize,
    last: DateTime<Utc>,
    scores: &mut [Score],
    channels: &HashMap<ChannelId, usize>,
) -> OsuResult<()> {
    for (idx, score) in (1..).zip(scores.iter_mut()).take(max) {
        // Skip if its an older score
        if score.created_at <= last {
            continue;
        }

        let requires_combo = score.map.as_ref().map_or(false, |m| {
            matches!(m.mode, GameMode::STD | GameMode::CTB) && m.max_combo.is_none()
        });

        if requires_combo {
            if let Err(why) = prepare_score(ctx, score).await {
                let report = Report::new(why).wrap_err("failed to fill in max combo for tracking");
                warn!("{:?}", report);

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
                    if let Err(why) = msg_fut.exec().await {
                        if let TwilightErrorType::Response { error, .. } = why.kind() {
                            if let ApiError::General(GeneralApiError {
                                code: ErrorCode::UnknownChannel,
                                ..
                            }) = error
                            {
                                let remove_fut =
                                    ctx.tracking().remove_channel(channel, None, ctx.psql());

                                if let Err(why) = remove_fut.await {
                                    let wrap = format!(
                                        "failed to remove osu tracks from unknown channel {}",
                                        channel
                                    );
                                    let report = Report::new(why).wrap_err(wrap);
                                    warn!("{:?}", report);
                                }
                            } else {
                                warn!(
                                    "Error from API while sending osu notif (channel {}): {}",
                                    channel, error
                                )
                            }
                        } else {
                            let wrap =
                                format!("error while sending osu notif (channel {})", channel);
                            let report = Report::new(why).wrap_err(wrap);
                            warn!("{:?}", report);
                        }
                    }
                }
                Err(why) => {
                    let report =
                        Report::new(why).wrap_err("invalid embed for osu!tracking notification");
                    warn!("{:?}", report);
                }
            }
        }
    }

    Ok(())
}

struct TrackUser<'u> {
    user_id: u32,
    mode: GameMode,
    user_ref: Option<&'u User>,
    user: Option<User>,
    embed: Option<Embed>,
}

impl<'u> TrackUser<'u> {
    #[inline]
    fn new(user_id: u32, mode: GameMode, user_ref: Option<&'u User>) -> Self {
        Self {
            user_id,
            mode,
            user_ref,
            user: None,
            embed: None,
        }
    }

    async fn embed(&mut self, ctx: &Context, score: &Score, idx: usize) -> OsuResult<Embed> {
        if let Some(ref embed) = self.embed {
            return Ok(embed.to_owned());
        }

        let data = if let Some(user) = self.user_ref {
            TrackNotificationEmbed::new(user, score, idx).await
        } else if let Some(ref user) = self.user {
            TrackNotificationEmbed::new(user, score, idx).await
        } else {
            let user = ctx.osu().user(self.user_id).mode(self.mode).await?;
            let user = self.user.get_or_insert(user);

            TrackNotificationEmbed::new(user, score, idx).await
        };

        let embed = data.into_builder().build();

        Ok(self.embed.get_or_insert(embed).to_owned())
    }
}
