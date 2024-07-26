use papaya::Operation;
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::Context;

impl Context {
    pub fn add_tracking(twitch_id: u64, channel_id: Id<ChannelMarker>) {
        Context::get()
            .data
            .tracked_streams
            .pin()
            .compute(twitch_id, |entry| match entry {
                Some((_, channels)) if channels.contains(&channel_id) => Operation::Abort(()),
                Some((_, old_channels)) => {
                    let mut new_channels = Vec::with_capacity(old_channels.len() + 1);
                    new_channels.extend_from_slice(old_channels);
                    new_channels.push(channel_id);

                    Operation::Insert(new_channels)
                }
                None => Operation::Insert(vec![channel_id]),
            });
    }

    pub fn remove_tracking(twitch_id: u64, channel_id: u64) {
        Context::get()
            .data
            .tracked_streams
            .pin()
            .update(twitch_id, |old_channels| {
                let mut new_channels = old_channels.clone();
                new_channels.retain(|&id| id != channel_id);

                new_channels
            });
    }

    pub fn tracked_users() -> Vec<u64> {
        Self::get()
            .data
            .tracked_streams
            .pin()
            .keys()
            .copied()
            .collect()
    }

    pub fn tracked_channels_for(twitch_id: u64) -> Option<Vec<Id<ChannelMarker>>> {
        Context::get()
            .data
            .tracked_streams
            .pin()
            .get(&twitch_id)
            .map(|channels| channels.to_vec())
    }

    pub fn tracked_users_in(channel: Id<ChannelMarker>) -> Vec<u64> {
        Context::get()
            .data
            .tracked_streams
            .pin()
            .iter()
            .filter_map(|(user, channels)| channels.contains(&channel).then_some(*user))
            .collect()
    }
}
