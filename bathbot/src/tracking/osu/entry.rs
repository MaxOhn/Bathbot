use std::{
    collections::HashMap,
    num::NonZeroU64,
    ops::Not,
    sync::{
        Arc, RwLock, RwLockReadGuard,
        atomic::{AtomicI64, AtomicU32, Ordering},
    },
};

use bathbot_psql::model::osu::DbTrackedOsuUser;
use bathbot_util::IntHasher;
use rosu_v2::{model::GameMode, prelude::Score};
use time::OffsetDateTime;

use super::TrackEntryParams;
use crate::core::Context;

type Channels = HashMap<NonZeroU64, TrackEntryParams, IntHasher>;

#[derive(Default)]
pub struct TrackEntry {
    /// Bits of the 100th score's pp value
    last_pp: AtomicU32,
    /// Unix timestamp of the last update
    last_ended_at: AtomicI64,
    channels: RwLock<Channels>,
}

impl TrackEntry {
    pub fn channels(&self) -> RwLockReadGuard<'_, Channels> {
        self.channels.read().unwrap()
    }

    /// Pp value of the 100th top score
    pub fn last_entry(&self) -> (f32, OffsetDateTime) {
        let pp = f32::from_bits(self.last_pp.load(Ordering::SeqCst));
        let timestamp = self.last_ended_at.load(Ordering::SeqCst);

        let last_updated = match OffsetDateTime::from_unix_timestamp(timestamp) {
            Ok(datetime) => datetime,
            Err(err) => {
                warn!(?err, "Invalid timestamp for datetime");

                OffsetDateTime::now_utc()
            }
        };

        (pp, last_updated)
    }

    fn is_empty(&self) -> bool {
        self.channels.read().unwrap().is_empty()
    }

    fn remove_channel(&self, channel_id: NonZeroU64) {
        self.channels.write().unwrap().remove(&channel_id);
    }

    pub fn add(&self, channel_id: NonZeroU64, params: TrackEntryParams) {
        self.channels.write().unwrap().insert(channel_id, params);
    }

    pub fn needs_last_pp(&self) -> bool {
        self.last_pp.load(Ordering::SeqCst) == 0
    }

    /// Stores the 100th score's pp value both in-memory and in the DB.
    pub async fn insert_last_pp(&self, user_id: u32, mode: GameMode, top_scores: &[Score]) {
        let pp = top_scores
            .last()
            .filter(|_| top_scores.len() == 100)
            .and_then(|score| score.pp)
            .unwrap_or(0.0);

        let last_ended_at = top_scores
            .iter()
            .map(|score| score.ended_at)
            .max()
            .unwrap_or_else(OffsetDateTime::now_utc);

        self.store_last_pp(pp, last_ended_at);
        let upsert_fut = Context::psql().upsert_tracked_last_pp(user_id, mode, pp, last_ended_at);

        if let Err(err) = upsert_fut.await {
            error!(
                user_id,
                ?mode,
                last_pp = pp,
                ?err,
                "Failed to upsert tracked last pp"
            );
        }
    }

    fn store_last_pp(&self, pp: f32, ended_at: OffsetDateTime) {
        self.last_pp.store(pp.to_bits(), Ordering::SeqCst);
        self.last_ended_at
            .store(ended_at.unix_timestamp(), Ordering::SeqCst);
    }

    fn insert(&self, user: DbTrackedOsuUser) {
        self.store_last_pp(user.last_pp, user.last_updated);

        let Some(channel_id) = NonZeroU64::new(user.channel_id as u64) else {
            return;
        };

        let params = TrackEntryParams::new()
            .with_index(
                user.min_index.map(|n| n as u8),
                user.max_index.map(|n| n as u8),
            )
            .with_pp(user.min_pp, user.max_pp)
            .with_combo_percent(user.min_combo_percent, user.max_combo_percent);

        self.channels.write().unwrap().insert(channel_id, params);
    }
}

#[derive(Clone, Default)]
pub struct TrackedUser {
    modes: [Arc<TrackEntry>; 4],
}

impl TrackedUser {
    /// Returns a user's [`TrackEntry`] if they're tracked in at least one
    /// channel for the [`GameMode`].
    pub fn try_get(&self, mode: GameMode) -> Option<Arc<TrackEntry>> {
        let entry = &self.modes[mode as usize];

        entry.is_empty().not().then(|| Arc::clone(entry))
    }

    /// Returns a user's [`TrackEntry`] for the [`GameMode`].
    pub fn get(&self, mode: GameMode) -> Arc<TrackEntry> {
        Arc::clone(&self.modes[mode as usize])
    }

    pub fn remove_channel(&self, channel_id: NonZeroU64, mode: Option<GameMode>) {
        if let Some(mode) = mode {
            if let Some(entry) = self.try_get(mode) {
                entry.remove_channel(channel_id);
            }
        } else {
            for entry in self.modes.iter() {
                entry.remove_channel(channel_id);
            }
        }
    }

    pub fn insert(&self, user: DbTrackedOsuUser) {
        self.modes[user.gamemode as usize].insert(user);
    }
}
