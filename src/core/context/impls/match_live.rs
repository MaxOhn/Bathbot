use crate::{
    commands::osu::process_match,
    embeds::{EmbedData, MatchCostEmbed, MatchLiveEmbed},
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
use twilight_model::{
    channel::{embed::Embed, Message},
    id::ChannelId,
};

// Not a DashSet as the list is expected to be
// very short and thus cheap to iterate over
type ChannelList = SmallVec<[ChannelId; 2]>;

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
    /// Failed to request match
    Error,
}

impl Context {
    /// In case the channel tracks exactly one match, returns the match's id
    pub fn tracks_single_match(&self, channel: ChannelId) -> Option<u32> {
        let match_live = &self.data.match_live;

        match_live
            .channel_count
            .get(&channel)
            .filter(|n| *n.value() == 1)?;

        match_live
            .match_channels
            .iter()
            .find(|entry| {
                let (_, channels) = entry.value();

                channels.iter().any(|&id| id == channel)
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
                let (_, channel_list) = e.get_mut();

                // The match is already tracked in the current channel
                if channel_list.iter().any(|&id| id == channel) {
                    // Undo the increment from above
                    match_live
                        .channel_count
                        .entry(channel)
                        .and_modify(|count| *count = count.saturating_sub(1));

                    MatchTrackResult::Duplicate
                } else {
                    channel_list.push(channel);

                    MatchTrackResult::Added
                }
            }
            // The match is not yet tracked -> request and store it
            Entry::Vacant(e) => match self.osu().osu_match(match_id).await {
                Ok(osu_match) => {
                    let tracked_match = TrackedMatch::new(osu_match, todo!());
                    e.insert((Mutex::new(tracked_match), smallvec![channel]));

                    MatchTrackResult::Added
                }
                Err(why) => {
                    unwind_error!(warn, why, "Failed to request initial match: {}");

                    MatchTrackResult::Error
                }
            },
        }
    }

    /// Returns false if the match wasnt tracked in the channel
    pub fn remove_match_track(&self, channel: ChannelId, match_id: u32) -> bool {
        let match_live = &self.data.match_live;

        if let Entry::Occupied(mut e) = match_live.match_channels.entry(match_id) {
            let (_, channels) = e.get_mut();

            // Check if the match is being tracked in the channel
            if let Some(idx) = channels.iter().position(|&id| id == channel) {
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

        if let Entry::Occupied(e) = match_live.match_channels.entry(match_id) {
            let (_, channels) = e.remove();

            for channel in channels {
                match_live
                    .channel_count
                    .entry(channel)
                    .and_modify(|count| *count = count.saturating_sub(1));
            }
        }
    }

    pub async fn match_live_loop(ctx: Arc<Context>) {
        // if cfg!(debug_assertions) {
        //     info!("Skip osu! tracking on debug");

        //     return;
        // }

        let mut interval = interval(Duration::from_secs(10));
        interval.tick().await;

        loop {
            interval.tick().await;

            // Match ids of matches that finished this iteration
            let mut remove = SmallVec::<[u32; 2]>::new();

            for entry in ctx.data.match_live.match_channels.iter() {
                let (locked_match, channels) = entry.value();

                let mut tracked_match = locked_match.lock().await;
                let match_id = tracked_match.osu_match.match_id;

                let mut next_match = match tracked_match.osu_match.get_next(ctx.osu()).await {
                    Ok(next_match) => next_match,
                    Err(_) => {
                        // Quick nap before trying again
                        sleep(Duration::from_millis(500)).await;

                        match tracked_match.osu_match.get_next(ctx.osu()).await {
                            Ok(next_match) => next_match,
                            Err(why) => {
                                unwind_error!(
                                    warn,
                                    why,
                                    "Failed to request match id {}: {}",
                                    match_id
                                );

                                continue;
                            }
                        }
                    }
                };

                std::mem::swap(&mut tracked_match.osu_match, &mut next_match);

                match tracked_match.all_events {
                    Some(ref mut all_events) => all_events.events.append(&mut next_match.events),
                    None => {
                        tracked_match.all_events.replace(next_match);
                    }
                }

                // If the same *ongoing* game is still the last event
                // or if there are no new events, continue
                if let Some(event) = tracked_match.osu_match.events.last() {
                    if let MatchEvent::Game { game, .. } = event {
                        if game.end_time.is_none() {
                            let same_game = tracked_match
                                .all_events
                                .as_ref()
                                .and_then(|m| m.games().next_back())
                                .map_or(false, |prev_game| game.game_id == prev_game.game_id);

                            if same_game {
                                continue;
                            }
                        }
                    }
                } else {
                    continue;
                }

                // let embeds = MatchLiveEmbed::new(&tracked_match.osu_match);

                for channel in channels.iter() {
                    //     let embed = match embed.build().build() {
                    //         Ok(embed) => embed,
                    //         Err(why) => {
                    //             warn!("Error while creating match live embed: {}", why);

                    //             continue;
                    //         }
                    //     };

                    //     let fut = match ctx.http.create_message(channel).embed(embed) {
                    //         Ok(fut) => fut,
                    //         Err(why) => {
                    //             warn!("Error while creating match live message: {}", why);

                    //             continue;
                    //         }
                    //     };

                    //     if let Err(why) = fut.await {
                    //         warn!("Error while sending match live message: {}", why)
                    //     }
                    // }
                }

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

            // Remove the match id entry
            for match_id in remove {
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

    for &channel in channels.iter() {
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
    osu_match: OsuMatch,
    all_events: Option<OsuMatch>,
    embed: Embed,
}

impl TrackedMatch {
    #[inline]
    fn new(osu_match: OsuMatch, embed: Embed) -> Self {
        Self {
            osu_match,
            all_events: None,
            embed,
        }
    }
}
