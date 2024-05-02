use std::{slice, time::Duration};

use eyre::{Context as EyreContext, Result};
use tokio::time::{interval, MissedTickBehavior};
use twilight_model::id::{
    marker::{ChannelMarker, MessageMarker},
    Id,
};

pub use self::types::*;
use crate::{core::Context, embeds::MatchLiveEmbed};

mod types;

const EMBED_LIMIT: usize = 10;

/// Sends a message to the channel for each embed
/// and returns the last of these messages
pub async fn send_match_messages(
    channel: Id<ChannelMarker>,
    embeds: &[MatchLiveEmbed],
) -> Result<Id<MessageMarker>> {
    let mut iter = embeds.iter();

    // Msg of last embed will be stored, do it separately
    let last = iter
        .next_back()
        .expect("no embed on fresh match")
        .as_embed();

    let http = Context::http();

    let mut last_msg_fut = http
        .create_message(channel)
        .embeds(slice::from_ref(&last))
        .wrap_err("Failed to create last match live msg")?;

    if embeds.len() <= EMBED_LIMIT {
        let mut interval = interval(Duration::from_millis(250));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        for embed in iter {
            let embed = embed.as_embed();
            interval.tick().await;

            match http.create_message(channel).embeds(&[embed]) {
                Ok(msg_fut) => {
                    if let Err(err) = msg_fut.await {
                        warn!(?err, "Failed to send match live embed");
                    }
                }
                Err(err) => {
                    warn!(?err, "Failed to create match live msg");
                }
            }
        }
    } else {
        last_msg_fut = last_msg_fut
            .content("The match has been going too long for me to send all previous messages.")
            .unwrap();
    }

    let last_msg = last_msg_fut
        .await
        .wrap_err("Failed to send last match live embed")?
        .model()
        .await
        .wrap_err("Failed to deserialize last match live embed response")?;

    Ok(last_msg.id)
}
