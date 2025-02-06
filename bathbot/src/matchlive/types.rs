use std::collections::HashMap;

use bathbot_util::IntHasher;
use rosu_v2::prelude::OsuMatch;
use smallvec::SmallVec;
use tokio::sync::Mutex;
use twilight_model::id::{
    marker::{ChannelMarker, MessageMarker},
    Id,
};

use crate::embeds::{MatchLiveEmbed, MatchLiveEmbeds};

pub struct MatchLiveChannels {
    // use tokio's mutex because it locks across futures
    pub inner: Mutex<MatchLiveChannelsInner>,
}

impl MatchLiveChannels {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MatchLiveChannelsInner::default()),
        }
    }
}

#[derive(Default)]
pub struct MatchLiveChannelsInner {
    /// Mapping match ids to channels that track them
    pub match_channels: HashMap<u32, MatchEntry, IntHasher>,

    /// Mapping channels to the amount of tracked matches in that channel
    pub channel_count: HashMap<Id<ChannelMarker>, u8, IntHasher>,
}

pub struct MatchEntry {
    pub tracked: TrackedMatch,
    // Not a set since the list is expected to be very short and thus cheap to iterate over.
    /// Channels that are tracking the match
    pub channels: SmallVec<[Channel; 2]>,
}

impl MatchEntry {
    pub fn new(tracked: TrackedMatch, channel: Channel) -> Self {
        Self {
            tracked,
            channels: smallvec::smallvec![channel],
        }
    }
}

pub struct Channel {
    pub id: Id<ChannelMarker>,
    /// Last msg in the channel
    pub msg_id: Id<MessageMarker>,
}

impl Channel {
    pub fn new(id: Id<ChannelMarker>, msg_id: Id<MessageMarker>) -> Self {
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

pub struct TrackedMatch {
    /// Most recent update of the match
    pub osu_match: OsuMatch,
    /// All embeds of the match
    pub embeds: Vec<MatchLiveEmbed>,
}

impl TrackedMatch {
    pub fn new(osu_match: OsuMatch, embeds: MatchLiveEmbeds) -> Self {
        Self {
            osu_match,
            embeds: embeds.into_vec(),
        }
    }
}
