use std::iter;

use twilight_model::id::{marker::ChannelMarker, Id};

use crate::Context;

impl Context {
    pub fn add_tracking(twitch_id: u64, channel_id: Id<ChannelMarker>) {
        let streams = &Context::get().data.tracked_streams;
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

    pub fn remove_tracking(twitch_id: u64, channel_id: u64) {
        Context::get()
            .data
            .tracked_streams
            .pin()
            .compute_if_present(&twitch_id, |_, channels| {
                let channels = channels.iter().copied().filter(|&id| id != channel_id);

                Some(channels.collect())
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
