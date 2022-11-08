use std::iter;

use crate::Context;

use twilight_model::id::{marker::ChannelMarker, Id};

impl Context {
    pub fn add_tracking(&self, twitch_id: u64, channel_id: Id<ChannelMarker>) {
        let streams = &self.data.tracked_streams;
        let guard = streams.guard();

        let missing = streams
            .compute_if_present(
                &twitch_id,
                |_, channels| {
                    let channels = channels.iter().copied().chain(iter::once(channel_id));

                    Some(channels.collect())
                },
                &guard,
            )
            .is_none();

        if missing {
            streams.insert(twitch_id, vec![channel_id], &guard);
        }
    }

    pub fn remove_tracking(&self, twitch_id: u64, channel_id: u64) {
        self.data
            .tracked_streams
            .pin()
            .compute_if_present(&twitch_id, |_, channels| {
                let channels = channels.iter().copied().filter(|&id| id != channel_id);

                Some(channels.collect())
            });
    }

    pub fn tracked_users(&self) -> Vec<u64> {
        self.data.tracked_streams.pin().keys().copied().collect()
    }

    pub fn tracked_channels_for(&self, twitch_id: u64) -> Option<Vec<Id<ChannelMarker>>> {
        self.data
            .tracked_streams
            .pin()
            .get(&twitch_id)
            .map(|channels| channels.to_vec())
    }

    pub fn tracked_users_in(&self, channel: Id<ChannelMarker>) -> Vec<u64> {
        self.data
            .tracked_streams
            .pin()
            .iter()
            .filter_map(|(user, channels)| channels.contains(&channel).then_some(*user))
            .collect()
    }
}
