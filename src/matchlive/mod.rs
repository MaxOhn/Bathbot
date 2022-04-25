use std::{slice, time::Duration};

use eyre::{Context as EyreContext, Report, Result};
use tokio::time::{interval, MissedTickBehavior};
use twilight_model::id::{
    marker::{ChannelMarker, MessageMarker},
    Id,
};

use crate::{core::Context, embeds::MatchLiveEmbed};

pub use self::types::*;

mod types;

const EMBED_LIMIT: usize = 10;

/// Sends a message to the channel for each embed
/// and returns the last of these messages
pub async fn send_match_messages(
    ctx: &Context,
    channel: Id<ChannelMarker>,
    embeds: &[MatchLiveEmbed],
) -> Result<Id<MessageMarker>> {
    let mut iter = embeds.iter();

    // Msg of last embed will be stored, do it separately
    let last = iter
        .next_back()
        .expect("no embed on fresh match")
        .as_embed();

    let mut last_msg_fut = ctx
        .http
        .create_message(channel)
        .embeds(slice::from_ref(&last))
        .wrap_err("failed to create last match live msg")?;

    if embeds.len() <= EMBED_LIMIT {
        let mut interval = interval(Duration::from_millis(250));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        for embed in iter {
            let embed = embed.as_embed();
            interval.tick().await;

            match ctx.http.create_message(channel).embeds(&[embed]) {
                Ok(msg_fut) => {
                    if let Err(err) = msg_fut.exec().await {
                        let report =
                            Report::new(err).wrap_err("error while sending match live embed");
                        warn!("{report:?}");
                    }
                }
                Err(err) => {
                    let report = Report::new(err).wrap_err("error while creating match live msg");
                    warn!("{report:?}");
                }
            }
        }
    } else {
        last_msg_fut = last_msg_fut
            .content("The match has been going too long for me to send all previous messages.")
            .unwrap();
    }

    let last_msg = last_msg_fut
        .exec()
        .await
        .wrap_err("failed to send last match live embed")?
        .model()
        .await
        .wrap_err("failed to deserialize last match live embed response")?;

    Ok(last_msg.id)
}
