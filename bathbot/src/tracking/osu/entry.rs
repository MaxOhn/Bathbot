use std::{
    num::NonZeroU64,
    ops::Not,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use bathbot_psql::model::osu::DbTrackedOsuUser;
use bathbot_util::IntHasher;
use papaya::{Guard, HashMap as PapayaMap, Iter, OwnedGuard};
use rosu_v2::{model::GameMode, prelude::Score};

use super::TrackEntryParams;
use crate::core::Context;

#[derive(Default)]
pub struct TrackEntry {
    /// Bits of the 100th score's pp value
    last_pp: AtomicU32,
    // Instead of `PapayaMap` we could use `RwLock<HashMap>` for less internal
    // book-keeping and auxiliary data which is probably more desirable
    // considering we'll store many instances but access will be asynchronous
    // which prevents some closure usage so we'll stick with the former.
    channels: PapayaMap<NonZeroU64, TrackEntryParams, IntHasher>,
}

impl TrackEntry {
    pub fn guard_channels(&self) -> OwnedGuard<'_> {
        self.channels.owned_guard()
    }

    pub fn iter_channels<'g, G: Guard>(
        &self,
        guard: &'g G,
    ) -> Iter<'g, NonZeroU64, TrackEntryParams, G> {
        self.channels.iter(guard)
    }

    /// Pp value of the 100th top score
    pub fn last_pp(&self) -> f32 {
        f32::from_bits(self.last_pp.load(Ordering::SeqCst))
    }

    fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    fn remove_channel(&self, channel_id: NonZeroU64) {
        self.channels.pin().remove(&channel_id);
    }

    fn add(&self, channel_id: NonZeroU64, params: TrackEntryParams) {
        self.channels.pin().insert(channel_id, params);
    }

    fn needs_last_pp(&self) -> bool {
        self.last_pp.load(Ordering::SeqCst) == 0
    }

    /// Stores the 100th score's pp value both in-memory and in the DB.
    pub async fn insert_last_pp(&self, user_id: u32, mode: GameMode, top_scores: &[Score]) {
        let pp = top_scores
            .last()
            .filter(|_| top_scores.len() == 100)
            .and_then(|score| score.pp)
            .unwrap_or(0.0);

        self.store_last_pp(pp);
        let upsert_fut = Context::psql().upsert_tracked_last_pp(user_id, mode, pp);

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

    fn store_last_pp(&self, pp: f32) {
        self.last_pp.store(pp.to_bits(), Ordering::SeqCst);
    }

    fn clear(&self) {
        self.last_pp.store(0, Ordering::SeqCst);
        self.channels.pin().clear();
    }

    fn insert(&self, user: DbTrackedOsuUser) {
        self.store_last_pp(user.last_pp);

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

        self.channels.pin().insert(channel_id, params);
    }
}

#[derive(Clone, Default)]
pub struct TrackedUser {
    modes: [Arc<TrackEntry>; 4],
}

impl TrackedUser {
    pub fn get(&self, mode: GameMode) -> Option<Arc<TrackEntry>> {
        let entry = &self.modes[mode as usize];

        entry.is_empty().not().then(|| Arc::clone(entry))
    }

    pub fn is_empty(&self) -> bool {
        self.modes.iter().all(|entry| entry.is_empty())
    }

    pub fn remove_channel(&self, channel_id: NonZeroU64, mode: Option<GameMode>) {
        if let Some(mode) = mode {
            if let Some(entry) = self.get(mode) {
                entry.remove_channel(channel_id);
            }
        } else {
            for entry in self.modes.iter() {
                entry.remove_channel(channel_id)
            }
        }
    }

    pub fn add(&self, mode: GameMode, channel_id: NonZeroU64, params: TrackEntryParams) {
        self.modes[mode as usize].add(channel_id, params);
    }

    pub fn needs_last_pp(&self, mode: GameMode) -> bool {
        self.modes[mode as usize].needs_last_pp()
    }

    pub async fn insert_last_pp(&self, user_id: u32, mode: GameMode, top_scores: &[Score]) {
        self.modes[mode as usize]
            .insert_last_pp(user_id, mode, top_scores)
            .await
    }

    pub fn clear(&self, mode: GameMode) {
        self.modes[mode as usize].clear();
    }

    pub fn insert(&self, user: DbTrackedOsuUser) {
        self.modes[user.gamemode as usize].insert(user);
    }
}
