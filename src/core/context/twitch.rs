use crate::Context;

use twilight_model::id::{marker::ChannelMarker, Id};

impl Context {
    pub fn add_tracking(&self, twitch_id: u64, channel_id: u64) {
        self.data
            .tracked_streams
            .entry(twitch_id)
            .or_default()
            .push(channel_id);
    }

    pub fn remove_tracking(&self, twitch_id: u64, channel_id: u64) {
        self.data
            .tracked_streams
            .entry(twitch_id)
            .and_modify(|channels| {
                if let Some(idx) = channels.iter().position(|&id| id == channel_id) {
                    channels.swap_remove(idx);
                };
            });
    }

    pub fn tracked_users(&self) -> Vec<u64> {
        self.data
            .tracked_streams
            .iter()
            .map(|guard| *guard.key())
            .collect()
    }

    pub fn tracked_channels_for(&self, twitch_id: u64) -> Option<Vec<Id<ChannelMarker>>> {
        self.data.tracked_streams.get(&twitch_id).map(|guard| {
            guard
                .value()
                .iter()
                .map(|&channel| Id::new(channel))
                .collect()
        })
    }

    pub fn tracked_users_in(&self, channel: Id<ChannelMarker>) -> Vec<u64> {
        self.data
            .tracked_streams
            .iter()
            .filter_map(|guard| guard.value().contains(&channel.get()).then(|| *guard.key()))
            .collect()
    }
}
