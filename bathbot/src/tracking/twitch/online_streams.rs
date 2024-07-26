use std::borrow::Borrow;

use bathbot_model::TwitchStream;
use bathbot_util::IntHasher;
use papaya::{Guard, HashMap as PapayaMap};

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct TwitchUserId(u64);

impl From<u64> for TwitchUserId {
    fn from(user_id: u64) -> Self {
        Self(user_id)
    }
}

impl Borrow<u64> for TwitchUserId {
    fn borrow(&self) -> &u64 {
        &self.0
    }
}

pub struct TwitchStreamId(
    // false positive; used when logging
    #[allow(unused)] u64,
);

impl From<u64> for TwitchStreamId {
    fn from(stream_id: u64) -> Self {
        Self(stream_id)
    }
}

#[derive(Default)]
pub struct OnlineTwitchStreams {
    user_streams: PapayaMap<TwitchUserId, TwitchStreamId, IntHasher>,
}

impl OnlineTwitchStreams {
    pub fn guard(&self) -> impl Guard + '_ {
        self.user_streams.guard()
    }

    pub fn is_user_online(&self, user: u64) -> bool {
        self.user_streams.pin().contains_key(&user)
    }

    pub fn set_online(&self, stream: &TwitchStream, guard: &impl Guard) {
        self.user_streams
            .insert(stream.user_id.into(), stream.stream_id.into(), guard);
    }

    pub fn set_offline(&self, stream: &TwitchStream, guard: &impl Guard) {
        self.user_streams.remove(&stream.user_id, guard);
    }

    pub fn set_offline_by_user(&self, user: u64) {
        self.user_streams.pin().remove(&user);
    }
}
