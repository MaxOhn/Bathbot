use crate::Context;

use rayon::prelude::*;
use twilight_model::id::ChannelId;

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
                    channels.remove(idx);
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

    pub fn tracked_channels_for(&self, twitch_id: u64) -> Option<Vec<ChannelId>> {
        self.data.tracked_streams.get(&twitch_id).map(|guard| {
            guard
                .value()
                .iter()
                .map(|&channel| ChannelId(channel))
                .collect()
        })
    }

    pub fn tracked_users_in(&self, channel: ChannelId) -> Vec<u64> {
        self.data
            .tracked_streams
            .iter()
            .par_bridge()
            .filter_map(|guard| {
                if guard.value().contains(&channel.0) {
                    Some(*guard.key())
                } else {
                    None
                }
            })
            .collect()
    }
}
