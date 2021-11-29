use dashmap::{mapref::entry::Entry, DashMap};
use eyre::Report;
use parking_lot::Mutex;
use rosu_v2::prelude::{MatchEvent, OsuError, OsuMatch};
use smallvec::SmallVec;
use std::sync::Arc;
use tokio::time::{interval, sleep, Duration};
use twilight_model::id::{ChannelId, MessageId};

use crate::{
    embeds::{EmbedData, MatchLiveEmbed, MatchLiveEmbeds},
    Context,
};

// Not a DashSet as the list is expected to be
// very short and thus cheap to iterate over.
// Tuple contains the channel id, as well as the msg id
// of the last msg in the channel.
type ChannelList = SmallVec<[(ChannelId, Mutex<MessageId>); 2]>;

pub struct MatchLiveChannels {
    /// Mapping match ids to channels that track them
    match_channels: DashMap<u32, (Mutex<TrackedMatch>, ChannelList)>,

    /// Mapping channels to the amount of tracked matches in that channel
    channel_count: DashMap<ChannelId, u8>,
}

impl MatchLiveChannels {
    pub fn new() -> Self {
        Self {
            match_channels: DashMap::new(),
            channel_count: DashMap::new(),
        }
    }
}

pub enum MatchTrackResult {
    /// The match id is now tracked in the channel
    Added,
    /// The channel tracks already three matches
    Capped,
    /// The match id was already tracked in the channel
    Duplicate,
    /// Failed to request match or send the embed messages
    Error,
    /// The match is private
    Private,
}

const EMBED_LIMIT: usize = 10;

impl Context {
    /// In case the channel tracks exactly one match, returns the match's id
    pub fn tracks_single_match(&self, channel: ChannelId) -> Option<u32> {
        let match_live = &self.data.match_live;

        // If the channel doesn't track exactly one match, return early
        match_live
            .channel_count
            .get(&channel)
            .filter(|n| *n.value() == 1)?;

        // Get the first match id tracked by the channel
        match_live
            .match_channels
            .iter()
            .find(|entry| {
                let (_, channels) = entry.value();

                channels.iter().any(|&(id, _)| id == channel)
            })
            .map(|entry| *entry.key())
    }

    pub async fn add_match_track(&self, channel: ChannelId, match_id: u32) -> MatchTrackResult {
        let match_live = &self.data.match_live;

        // Increment the track counter for the channel
        let capped = match_live
            .channel_count
            .get(&channel)
            .map_or(false, |entry| *entry.value() >= 3);

        // Return early if channel is already tracking three channels
        if capped {
            return MatchTrackResult::Capped;
        }

        match match_live.match_channels.entry(match_id) {
            // The match is already being tracked in some channel
            Entry::Occupied(mut e) => {
                let (tracked_match, channel_list) = e.get_mut();

                // The match is already tracked in the current channel
                if channel_list.iter().any(|&(id, _)| id == channel) {
                    return MatchTrackResult::Duplicate;
                }

                let locked_match = tracked_match.lock();

                let msg = match send_match_messages(self, channel, &locked_match.embeds).await {
                    Some(msg) => Mutex::new(msg),
                    None => return MatchTrackResult::Error,
                };

                *match_live.channel_count.entry(channel).or_insert(0) += 1;
                channel_list.push((channel, msg));

                MatchTrackResult::Added
            }
            // The match is not yet tracked -> request and store it
            Entry::Vacant(e) => match self.osu().osu_match(match_id).await {
                Ok(osu_match) => {
                    let embeds = MatchLiveEmbed::new(&osu_match);

                    let msg = match send_match_messages(self, channel, &embeds).await {
                        Some(msg) => Mutex::new(msg),
                        None => return MatchTrackResult::Error,
                    };

                    // Only add to tracking if it's not already disbanded
                    if !matches!(osu_match.events.last(), Some(MatchEvent::Disbanded { .. })) {
                        *match_live.channel_count.entry(channel).or_insert(0) += 1;
                        let tracked_match = TrackedMatch::new(osu_match, embeds);
                        e.insert((Mutex::new(tracked_match), smallvec![(channel, msg)]));
                    }

                    MatchTrackResult::Added
                }
                Err(OsuError::Response { status, .. }) if status.as_u16() == 401 => {
                    MatchTrackResult::Private
                }
                Err(why) => {
                    let report = Report::new(why).wrap_err("failed to request initial match");
                    warn!("{:?}", report);

                    MatchTrackResult::Error
                }
            },
        }
    }

    /// Returns false if the match wasn't tracked in the channel
    pub fn remove_match_track(&self, channel: ChannelId, match_id: u32) -> bool {
        let match_live = &self.data.match_live;

        if let Entry::Occupied(mut e) = match_live.match_channels.entry(match_id) {
            let (_, channels) = e.get_mut();

            // Check if the match is being tracked in the channel
            if let Some(idx) = channels.iter().position(|&(id, _)| id == channel) {
                channels.swap_remove(idx);

                // Decrement the counter for the channel
                match_live
                    .channel_count
                    .entry(channel)
                    .and_modify(|count| *count = count.saturating_sub(1));

                // If no channel is tracking the match, remove the entry
                if channels.is_empty() {
                    e.remove();
                }

                return true;
            }
        }

        false
    }

    fn remove_all_match_tracks(&self, match_id: u32) {
        let match_live = &self.data.match_live;

        if let Some((_, (_, channels))) = match_live.match_channels.remove(&match_id) {
            for (channel, _) in channels {
                match_live
                    .channel_count
                    .entry(channel)
                    .and_modify(|count| *count = count.saturating_sub(1));
            }
        }
    }

    pub async fn match_live_loop(ctx: Arc<Context>) {
        if cfg!(debug_assertions) {
            info!("Skip match live tracking on debug");

            return;
        }

        let mut interval = interval(Duration::from_secs(10));
        interval.tick().await;

        // Match ids of matches that finished this iteration
        let mut remove = Vec::new();

        loop {
            interval.tick().await;

            for entry in ctx.data.match_live.match_channels.iter() {
                let (locked_match, channels) = entry.value();
                let mut tracked_match = locked_match.lock();

                let next_match = match tracked_match.osu_match.get_next(ctx.osu()).await {
                    Ok(next_match) => next_match,
                    Err(why) => {
                        let report =
                            Report::new(why).wrap_err("failed to request match for live ticker");
                        warn!("{:?}", report);

                        continue;
                    }
                };

                let (update, new_embeds) = tracked_match
                    .embeds
                    .last_mut()
                    .expect("no last live embed")
                    .update(&next_match);

                if next_match.end_time.is_some() {
                    remove.push(next_match.match_id);
                }

                tracked_match.osu_match = next_match;

                if update {
                    let data = tracked_match.embeds.last().unwrap();

                    for (channel, msg) in channels.iter() {
                        let msg = *msg.lock();
                        let embed = &[data.as_builder().build()];
                        let update_result = ctx.http.update_message(*channel, msg).embeds(embed);

                        let update_fut = match update_result {
                            Ok(update_fut) => update_fut.exec(),
                            Err(why) => {
                                let report = Report::new(why)
                                    .wrap_err("failed to build msg update for live match");
                                warn!("{:?}", report);

                                continue;
                            }
                        };

                        if let Err(why) = update_fut.await {
                            let report =
                                Report::new(why).wrap_err("failed to update match live msg");
                            warn!("{:?}", report);
                        }
                    }
                }

                if let Some(embeds) = new_embeds {
                    for (channel, msg_lock) in channels.iter() {
                        match send_match_messages(&ctx, *channel, &embeds).await {
                            Some(msg) => *msg_lock.lock() = msg,
                            None => error!("Failed to send last match live message"),
                        }
                    }

                    tracked_match.embeds.extend(embeds);
                }
            }

            // Remove the match id entries
            for match_id in remove.drain(..) {
                ctx.remove_all_match_tracks(match_id);
            }
        }
    }

    pub async fn notify_match_live_shutdown(&self) -> usize {
        let match_live = &self.data.match_live;
        match_live.match_channels.clear();

        let content = "I'm about to reboot so the match tracking will be aborted, \
            you can restart it in just a moment...";

        let mut notified = 0;

        for entry in match_live.channel_count.iter() {
            if *entry.value() > 0 {
                let _ = self
                    .http
                    .create_message(*entry.key())
                    .content(content)
                    .unwrap()
                    .exec()
                    .await;

                notified += 1;
            }
        }

        match_live.channel_count.clear();

        notified
    }
}

struct TrackedMatch {
    /// Most recent update of the match
    osu_match: OsuMatch,
    /// All embeds of the match
    embeds: Vec<MatchLiveEmbed>,
}

impl TrackedMatch {
    fn new(osu_match: OsuMatch, embeds: MatchLiveEmbeds) -> Self {
        Self {
            osu_match,
            embeds: embeds.into_vec(),
        }
    }
}

/// Sends a message to the channel for each embed
/// and returns the last of these messages
async fn send_match_messages(
    ctx: &Context,
    channel: ChannelId,
    embeds: &[MatchLiveEmbed],
) -> Option<MessageId> {
    let mut iter = embeds.iter();

    // Msg of last embed will be stored, do it separately
    let last = iter.next_back().expect("no embed on fresh match");

    let content = if embeds.len() <= EMBED_LIMIT {
        for embed in iter {
            let embed = embed.as_builder().build();

            match ctx.http.create_message(channel).embeds(&[embed]) {
                Ok(msg_fut) => {
                    if let Err(why) = msg_fut.exec().await {
                        let report =
                            Report::new(why).wrap_err("error while sending match live embed");
                        warn!("{:?}", report);
                    }
                }
                Err(why) => {
                    let report = Report::new(why).wrap_err("error while creating match live msg");
                    warn!("{:?}", report);
                }
            }

            sleep(Duration::from_millis(250)).await;
        }

        None
    } else {
        Some(
            "The match has been going too long \
            for me to send all previous messages.",
        )
    };

    let last = last.as_builder().build();

    match ctx.http.create_message(channel).embeds(&[last]) {
        Ok(msg_fut) => {
            let msg_fut = match content {
                Some(content) => msg_fut.content(content).unwrap(),
                None => msg_fut,
            };

            match msg_fut.exec().await {
                Ok(msg_res) => match msg_res.model().await {
                    Ok(msg) => Some(msg.id),
                    Err(why) => {
                        let report = Report::new(why)
                            .wrap_err("failed to deserialize last match live embed response");
                        error!("{:?}", report);

                        None
                    }
                },
                Err(why) => {
                    let report = Report::new(why).wrap_err("failed to send last match live embed");
                    error!("{:?}", report);

                    None
                }
            }
        }
        Err(why) => {
            let report = Report::new(why).wrap_err("failed to create last match live msg");
            error!("{:?}", report);

            None
        }
    }
}
