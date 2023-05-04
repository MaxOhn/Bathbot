#![cfg(feature = "matchlive")]

use std::{slice, sync::Arc};

use hashbrown::hash_map::Entry;
use rosu_v2::prelude::{MatchEvent, OsuError};
use tokio::time::{interval, Duration};
use twilight_model::id::{marker::ChannelMarker, Id};

use super::Context;
use crate::{
    embeds::MatchLiveEmbed,
    matchlive::{send_match_messages, Channel, MatchEntry, MatchTrackResult, TrackedMatch},
    util::ChannelExt,
};

impl Context {
    /// In case the channel tracks exactly one match, returns the match's id
    pub async fn tracks_single_match(&self, channel: Id<ChannelMarker>) -> Option<u32> {
        let match_live = self.data.matchlive.inner.lock().await;

        // If the channel doesn't track exactly one match, return early
        match_live
            .channel_count
            .get(&channel)
            .filter(|&count| *count == 1)?;

        // Get the first match id tracked by the channel
        match_live
            .match_channels
            .iter()
            .find(|(_, entry)| entry.channels.iter().any(|c| c.id == channel))
            .map(|(key, _)| *key)
    }

    pub async fn add_match_track(
        &self,
        channel: Id<ChannelMarker>,
        match_id: u32,
    ) -> MatchTrackResult {
        let mut match_live = self.data.matchlive.inner.lock().await;

        // Increment the track counter for the channel
        let capped = match_live
            .channel_count
            .get(&channel)
            .map_or(false, |count| *count >= 3);

        // Return early if channel is already tracking three channels
        if capped {
            return MatchTrackResult::Capped;
        }

        match match_live.match_channels.entry(match_id) {
            // The match is already being tracked in some channel
            Entry::Occupied(mut e) => {
                let entry = e.get_mut();

                // The match is already tracked in the current channel
                if entry.channels.iter().any(|c| c.id == channel) {
                    return MatchTrackResult::Duplicate;
                }

                let embeds = &entry.tracked.embeds;

                let channel = match send_match_messages(self, channel, embeds).await {
                    Ok(msg) => Channel::new(channel, msg),
                    Err(err) => {
                        error!("{err:?}");

                        return MatchTrackResult::Error;
                    }
                };

                let id = channel.id;
                entry.channels.push(channel);
                *match_live.channel_count.entry(id).or_insert(0) += 1;

                MatchTrackResult::Added
            }
            // The match is not yet tracked -> request and store it
            Entry::Vacant(e) => match self.osu().osu_match(match_id).await {
                Ok(osu_match) => {
                    let embeds = MatchLiveEmbed::new(&osu_match);

                    let channel = match send_match_messages(self, channel, &embeds).await {
                        Ok(msg) => Channel::new(channel, msg),
                        Err(err) => {
                            error!("{err:?}");

                            return MatchTrackResult::Error;
                        }
                    };

                    // Only add to tracking if it's not already disbanded
                    if !matches!(osu_match.events.last(), Some(MatchEvent::Disbanded { .. })) {
                        let tracked_match = TrackedMatch::new(osu_match, embeds);
                        let id = channel.id;
                        e.insert(MatchEntry::new(tracked_match, channel));
                        *match_live.channel_count.entry(id).or_insert(0) += 1;
                    }

                    MatchTrackResult::Added
                }
                Err(OsuError::NotFound) => MatchTrackResult::NotFound,
                Err(OsuError::Response { status, .. }) if status == 401 => {
                    MatchTrackResult::Private
                }
                Err(err) => {
                    warn!(?err, "Failed to request initial match");

                    MatchTrackResult::Error
                }
            },
        }
    }

    /// Returns false if the match wasn't tracked in the channel
    pub async fn remove_match_track(&self, channel: Id<ChannelMarker>, match_id: u32) -> bool {
        let mut match_live = self.data.matchlive.inner.lock().await;

        if let Entry::Occupied(mut e) = match_live.match_channels.entry(match_id) {
            let entry = e.get_mut();

            // Check if the match is being tracked in the channel
            if let Some(idx) = entry.channels.iter().position(|c| c.id == channel) {
                entry.channels.swap_remove(idx);

                // If no channel is tracking the match, remove the entry
                if entry.channels.is_empty() {
                    e.remove();
                }

                // Decrement the counter for the channel
                match_live
                    .channel_count
                    .entry(channel)
                    .and_modify(|count| *count -= 1);

                return true;
            }
        }

        false
    }

    /// Returns how many channels tracked the match before it ended
    async fn remove_all_match_tracks(&self, match_id: u32) -> usize {
        let mut match_live = self.data.matchlive.inner.lock().await;

        if let Some(entry) = match_live.match_channels.remove(&match_id) {
            for Channel { id, .. } in &entry.channels {
                match_live
                    .channel_count
                    .entry(*id)
                    .and_modify(|count| *count -= 1);
            }

            entry.channels.len()
        } else {
            0
        }
    }

    pub async fn match_live_loop(ctx: Arc<Context>) {
        // Update all matches every 10 seconds
        let mut interval = interval(Duration::from_secs(10));
        interval.tick().await;

        // Match ids of matches that finished this iteration
        let mut remove = Vec::new();

        loop {
            interval.tick().await;

            {
                // Tight scope makes sure this lock is dropped ASAP
                let mut match_live = ctx.data.matchlive.inner.lock().await;

                // For every match that is being tracked
                for entry in match_live.match_channels.values_mut() {
                    let mut tracked_match = &mut entry.tracked;

                    // Request an update
                    let next_match = match tracked_match.osu_match.get_next(ctx.osu()).await {
                        Ok(next_match) => next_match,
                        Err(err) => {
                            warn!(?err, "Failed to request match");

                            continue;
                        }
                    };

                    // Update the embeds
                    let (update, new_embeds) = tracked_match
                        .embeds
                        .last_mut()
                        .expect("no last live embed")
                        .update(&next_match);

                    if next_match.end_time.is_some() {
                        remove.push(next_match.match_id);
                    }

                    tracked_match.osu_match = next_match;

                    // If there was an update for the last embed
                    if update {
                        let data = tracked_match.embeds.last().unwrap();

                        // For every channel that's tracking the match
                        for Channel { id, msg_id } in entry.channels.iter() {
                            let embed = Some(data.as_embed());

                            // Update the last message
                            let update_result = ctx
                                .http
                                .update_message(*id, *msg_id)
                                .embeds(embed.as_ref().map(slice::from_ref));

                            let update_fut = match update_result {
                                Ok(update_fut) => update_fut,
                                Err(err) => {
                                    warn!(?err, "Failed to build msg update");

                                    continue;
                                }
                            };

                            if let Err(err) = update_fut.await {
                                warn!(?err, "Failed to update msg");
                            }
                        }
                    }

                    // For all new embeds, send them to all channels
                    if let Some(embeds) = new_embeds {
                        for Channel { id, msg_id } in entry.channels.iter_mut() {
                            match send_match_messages(&ctx, *id, &embeds).await {
                                Ok(msg) => *msg_id = msg,
                                Err(err) => {
                                    error!(channel = id.get(), ?err, "Failed to send last msg")
                                }
                            }
                        }

                        tracked_match.embeds.extend(embeds);
                    }
                }
            }

            // Remove the match id entries
            for match_id in remove.drain(..) {
                let count = ctx.remove_all_match_tracks(match_id).await;
                let plural = if count == 1 { "" } else { "s" };
                debug!("Match {match_id} over, removed from tracking for {count} channel{plural}");
            }
        }
    }

    pub async fn notify_match_live_shutdown(&self) -> usize {
        let mut match_live = self.data.matchlive.inner.lock().await;
        match_live.match_channels.clear();

        let content = "I'm about to reboot so the match tracking will be aborted, \
            you can restart it in just a moment...";

        let mut notified = 0;

        for (channel, count) in match_live.channel_count.iter() {
            if *count > 0 {
                let _ = channel.plain_message(self, content).await;
                notified += 1;
            }
        }

        match_live.channel_count.clear();

        notified
    }
}
