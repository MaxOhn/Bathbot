use std::{slice, sync::Arc};

use eyre::{Context as EyreContext, Report, Result};
use hashbrown::{hash_map::Entry, HashMap};
use rosu_v2::prelude::{MatchEvent, OsuError, OsuMatch};
use smallvec::SmallVec;
use tokio::{
    sync::Mutex,
    time::{interval, Duration, MissedTickBehavior},
};
use twilight_model::id::{
    marker::{ChannelMarker, MessageMarker},
    Id,
};

use crate::{
    embeds::{EmbedData, MatchLiveEmbed, MatchLiveEmbeds},
    Context,
};

pub struct MatchLiveChannels {
    inner: Mutex<MatchLiveChannelsInner>,
}

#[derive(Default)]
struct MatchLiveChannelsInner {
    /// Mapping match ids to channels that track them
    match_channels: HashMap<u32, MatchEntry>,

    /// Mapping channels to the amount of tracked matches in that channel
    channel_count: HashMap<Id<ChannelMarker>, u8>,
}

struct MatchEntry {
    tracked: TrackedMatch,
    // Not a set since the list is expected to be very short and thus cheap to iterate over.
    /// Channels that are tracking the match
    channels: SmallVec<[Channel; 2]>,
}

struct Channel {
    id: Id<ChannelMarker>,
    /// Last msg in the channel
    msg_id: Id<MessageMarker>,
}

impl MatchLiveChannels {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MatchLiveChannelsInner::default()),
        }
    }
}

impl MatchEntry {
    fn new(tracked: TrackedMatch, channel: Channel) -> Self {
        Self {
            tracked,
            channels: smallvec![channel],
        }
    }
}

impl Channel {
    fn new(id: Id<ChannelMarker>, msg_id: Id<MessageMarker>) -> Self {
        Self { id, msg_id }
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
    /// API does not know the match id
    NotFound,
    /// The match is private
    Private,
}

const EMBED_LIMIT: usize = 10;

impl Context {
    /// In case the channel tracks exactly one match, returns the match's id
    pub async fn tracks_single_match(&self, channel: Id<ChannelMarker>) -> Option<u32> {
        let match_live = self.data.match_live.inner.lock().await;

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
        let mut match_live = self.data.match_live.inner.lock().await;

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
                    Err(report) => {
                        error!("{report:?}");

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
                        Err(report) => {
                            error!("{report:?}");

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
                Err(why) => {
                    let report = Report::new(why).wrap_err("failed to request initial match");
                    warn!("{report:?}");

                    MatchTrackResult::Error
                }
            },
        }
    }

    /// Returns false if the match wasn't tracked in the channel
    pub async fn remove_match_track(&self, channel: Id<ChannelMarker>, match_id: u32) -> bool {
        let mut match_live = self.data.match_live.inner.lock().await;

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
        let mut match_live = self.data.match_live.inner.lock().await;

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

            {
                // Tight scope makes sure this lock is dropped ASAP
                let mut match_live = ctx.data.match_live.inner.lock().await;

                for entry in match_live.match_channels.values_mut() {
                    let mut tracked_match = &mut entry.tracked;

                    let next_match = match tracked_match.osu_match.get_next(ctx.osu()).await {
                        Ok(next_match) => next_match,
                        Err(err) => {
                            let report = Report::new(err).wrap_err("failed to request match");
                            warn!("{report:?}");

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

                        for Channel { id, msg_id } in entry.channels.iter() {
                            let embed = Some(data.as_builder().build());

                            let update_result = ctx
                                .http
                                .update_message(*id, *msg_id)
                                .embeds(embed.as_ref().map(slice::from_ref));

                            let update_fut = match update_result {
                                Ok(update_fut) => update_fut.exec(),
                                Err(err) => {
                                    let report =
                                        Report::new(err).wrap_err("failed to build msg update");
                                    warn!("{report:?}");

                                    continue;
                                }
                            };

                            if let Err(err) = update_fut.await {
                                let report = Report::new(err).wrap_err("failed to update msg");
                                warn!("{report:?}");
                            }
                        }
                    }

                    if let Some(embeds) = new_embeds {
                        for Channel { id, msg_id } in entry.channels.iter_mut() {
                            match send_match_messages(&ctx, *id, &embeds).await {
                                Ok(msg) => *msg_id = msg,
                                Err(report) => {
                                    let report = report.wrap_err(format!(
                                        "failed to send last msg in channel {id}"
                                    ));
                                    error!("{report:?}")
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
        let mut match_live = self.data.match_live.inner.lock().await;
        match_live.match_channels.clear();

        let content = "I'm about to reboot so the match tracking will be aborted, \
            you can restart it in just a moment...";

        let mut notified = 0;

        for (channel, count) in match_live.channel_count.iter() {
            if *count > 0 {
                let _ = self
                    .http
                    .create_message(*channel)
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
    channel: Id<ChannelMarker>,
    embeds: &[MatchLiveEmbed],
) -> Result<Id<MessageMarker>> {
    let mut iter = embeds.iter();

    // Msg of last embed will be stored, do it separately
    let last = iter
        .next_back()
        .expect("no embed on fresh match")
        .as_builder()
        .build();

    let mut last_msg_fut = ctx
        .http
        .create_message(channel)
        .embeds(slice::from_ref(&last))
        .wrap_err("failed to create last match live msg")?;

    if embeds.len() <= EMBED_LIMIT {
        let mut interval = interval(Duration::from_millis(250));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        for embed in iter {
            let embed = embed.as_builder().build();
            interval.tick().await;

            match ctx.http.create_message(channel).embeds(&[embed]) {
                Ok(msg_fut) => {
                    if let Err(why) = msg_fut.exec().await {
                        let report =
                            Report::new(why).wrap_err("error while sending match live embed");
                        warn!("{report:?}");
                    }
                }
                Err(why) => {
                    let report = Report::new(why).wrap_err("error while creating match live msg");
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
