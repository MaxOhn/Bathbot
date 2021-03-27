use crate::{
    commands::osu::process_match,
    embeds::{EmbedData, MatchCostEmbed, MatchLiveEmbed, MatchLiveEmbedUpdate, MatchLiveEmbeds},
    Context,
};

use dashmap::{mapref::entry::Entry, DashMap};
use rosu_v2::prelude::{MatchEvent, OsuMatch};
use smallvec::SmallVec;
use std::sync::Arc;
use tokio::{
    sync::Mutex,
    time::{interval, sleep, Duration},
};
use twilight_model::id::{ChannelId, MessageId};

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
}

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
        match match_live.channel_count.entry(channel) {
            Entry::Occupied(mut entry) => {
                // Return early if channel is already tracking three channels
                if *entry.get() >= 3 {
                    return MatchTrackResult::Capped;
                }

                *entry.get_mut() += 1;
            }
            Entry::Vacant(entry) => {
                entry.insert(1);
            }
        }

        match match_live.match_channels.entry(match_id) {
            // The match is already being tracked in some channel
            Entry::Occupied(mut e) => {
                let (tracked_match, channel_list) = e.get_mut();

                // The match is already tracked in the current channel
                if channel_list.iter().any(|&(id, _)| id == channel) {
                    // Undo the increment from above
                    match_live
                        .channel_count
                        .entry(channel)
                        .and_modify(|count| *count = count.saturating_sub(1));

                    MatchTrackResult::Duplicate
                } else {
                    let locked_match = tracked_match.lock().await;

                    let msg = match send_match_messages(self, channel, &locked_match.embeds).await {
                        Some(msg) => Mutex::new(msg),
                        None => return MatchTrackResult::Error,
                    };

                    channel_list.push((channel, msg));

                    MatchTrackResult::Added
                }
            }
            // The match is not yet tracked -> request and store it
            Entry::Vacant(e) => match self.osu().osu_match(match_id).await {
                Ok(osu_match) => {
                    let embeds = MatchLiveEmbed::new(&osu_match);

                    let msg = match send_match_messages(self, channel, &embeds).await {
                        Some(msg) => Mutex::new(msg),
                        None => return MatchTrackResult::Error,
                    };

                    let tracked_match = TrackedMatch::new(osu_match, embeds);
                    e.insert((Mutex::new(tracked_match), smallvec![(channel, msg)]));

                    MatchTrackResult::Added
                }
                Err(why) => {
                    unwind_error!(warn, why, "Failed to request initial match: {}");

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
                    drop(channels);
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
        // TODO
        // if cfg!(debug_assertions) {
        //     info!("Skip match live tracking on debug");

        //     return;
        // }

        let mut interval = interval(Duration::from_secs(10));
        interval.tick().await;

        // Match ids of matches that finished this iteration
        let mut remove = Vec::new();

        loop {
            interval.tick().await;

            for entry in ctx.data.match_live.match_channels.iter() {
                let (locked_match, channels) = entry.value();

                let mut tracked_match = locked_match.lock().await;
                let match_id = tracked_match.osu_match.match_id;

                let mut next_match = match tracked_match.osu_match.get_next(ctx.osu()).await {
                    Ok(next_match) => next_match,
                    Err(why) => {
                        unwind_error!(warn, why, "Failed to request match for live ticker: {}");

                        continue;
                    }
                };

                let (update, new_embeds) = tracked_match
                    .embeds
                    .last_mut()
                    .expect("no last live embed")
                    .update(&next_match);

                std::mem::swap(&mut tracked_match.osu_match, &mut next_match);

                // Put the previous game in all_events
                match tracked_match.all_events {
                    Some(ref mut all_events) => all_events.events.append(&mut next_match.events),
                    None => {
                        tracked_match.all_events.replace(next_match);
                    }
                }

                match update {
                    Some(MatchLiveEmbedUpdate::Modify) => {
                        let data = tracked_match.embeds.last().unwrap();

                        for (channel, msg) in channels.iter() {
                            let msg = *msg.lock().await;
                            let embed = data.as_builder().build();
                            let update_result = ctx.http.update_message(*channel, msg).embed(embed);

                            let update_fut = match update_result {
                                Ok(update_fut) => update_fut,
                                Err(why) => {
                                    unwind_error!(
                                        warn,
                                        why,
                                        "Failed to build msg update for live match: {}"
                                    );

                                    continue;
                                }
                            };

                            if let Err(why) = update_fut.await {
                                unwind_error!(warn, why, "Failed to update match live msg: {}");
                            }
                        }
                    }
                    Some(MatchLiveEmbedUpdate::Delete) => {
                        for (channel, msg) in channels.iter() {
                            let msg = msg.lock().await;

                            if let Err(why) = ctx.http.delete_message(*channel, *msg).await {
                                unwind_error!(warn, why, "Failed to delete match live msg: {}");
                            }
                        }
                    }
                    None => {}
                }

                if let Some(embeds) = new_embeds {
                    for (channel, msg_lock) in channels.iter() {
                        match send_match_messages(&ctx, *channel, &embeds).await {
                            Some(msg) => *msg_lock.lock().await = msg,
                            None => error!("Failed to send last match live message"),
                        }
                    }

                    tracked_match.embeds.extend(embeds);
                }

                // Check if match is over
                if tracked_match.osu_match.end_time.is_some() {
                    let mut all_events = tracked_match.all_events.take().unwrap();

                    all_events
                        .events
                        .append(&mut tracked_match.osu_match.events);

                    all_events.events.retain(|event| {
                        !matches!(event, MatchEvent::Game { game, .. } if game.end_time.is_none())
                    });

                    send_match_cost(&ctx, &mut tracked_match.osu_match, channels).await;

                    remove.push(match_id);
                }
            }

            // Remove the match id entries
            for match_id in remove.drain(..) {
                ctx.remove_all_match_tracks(match_id);
            }
        }
    }
}

async fn send_match_cost(ctx: &Context, osu_match: &mut OsuMatch, channels: &ChannelList) {
    let games: Vec<_> = osu_match.drain_games().collect();
    let match_result = Some(process_match(&games, true));

    let data = match MatchCostEmbed::new(&mut *osu_match, None, match_result) {
        Some(data) => data,
        None => return warn!("Match live embed could not be created"),
    };

    for &(channel, _) in channels.iter() {
        let embed = data.as_builder().build();

        let fut = match ctx.http.create_message(channel).embed(embed) {
            Ok(fut) => fut,
            Err(why) => return warn!("Error while creating match cost message: {}", why),
        };

        if let Err(why) = fut.await {
            warn!("Error while sending match cost message: {}", why)
        }
    }
}

struct TrackedMatch {
    /// Most recent update of the match
    osu_match: OsuMatch,
    /// Match containing all events *before* the most recent update
    all_events: Option<OsuMatch>,
    /// All embeds of the match
    embeds: Vec<MatchLiveEmbed>,
}

impl TrackedMatch {
    #[inline]
    fn new(osu_match: OsuMatch, embeds: MatchLiveEmbeds) -> Self {
        Self {
            osu_match,
            all_events: None,
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

    for embed in iter {
        let embed = embed.as_builder().build();

        match ctx.http.create_message(channel).embed(embed) {
            Ok(msg_fut) => {
                if let Err(why) = msg_fut.await {
                    unwind_error!(warn, why, "Error while sending match live embed: {}");
                }
            }
            Err(why) => {
                unwind_error!(warn, why, "Error while creating match live msg: {}");
            }
        }

        sleep(Duration::from_millis(250)).await;
    }

    let last = last.as_builder().build();

    match ctx.http.create_message(channel).embed(last) {
        Ok(msg_fut) => match msg_fut.await {
            Ok(msg) => Some(msg.id),
            Err(why) => {
                unwind_error!(error, why, "Failed to send last match live embed: {}");

                None
            }
        },
        Err(why) => {
            unwind_error!(error, why, "Failed to create last match live msg: {}");

            None
        }
    }
}
