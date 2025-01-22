use std::{
    collections::HashMap,
    num::NonZeroU64,
    ops::Not,
    sync::{
        atomic::{AtomicI64, AtomicU32, Ordering},
        Arc,
    },
};

use bathbot_psql::model::osu::DbTrackedOsuUser;
use bathbot_util::IntHasher;
use rosu_v2::{model::GameMode, prelude::Score};
use time::OffsetDateTime;
use tokio::sync::{RwLock, RwLockReadGuard};

use super::TrackEntryParams;
use crate::core::Context;

type Channels = HashMap<NonZeroU64, TrackEntryParams, IntHasher>;

#[derive(Default)]
pub struct TrackEntry {
    /// Bits of the 100th score's pp value
    last_pp: AtomicU32,
    /// Unix timestamp of the last update
    last_updated: AtomicI64,
    channels: RwLock<Channels>,
}

impl TrackEntry {
    pub async fn channels(&self) -> RwLockReadGuard<'_, Channels> {
        self.channels.read().await
    }

    /// Pp value of the 100th top score
    pub fn last_entry(&self) -> (f32, OffsetDateTime) {
        let pp = f32::from_bits(self.last_pp.load(Ordering::SeqCst));
        let timestamp = self.last_updated.load(Ordering::SeqCst);

        let last_updated = match OffsetDateTime::from_unix_timestamp(timestamp) {
            Ok(datetime) => datetime,
            Err(err) => {
                warn!(?err, "Invalid timestamp for datetime");

                OffsetDateTime::now_utc()
            }
        };

        (pp, last_updated)
    }

    async fn is_empty(&self) -> bool {
        self.channels.read().await.is_empty()
    }

    async fn remove_channel(&self, channel_id: NonZeroU64) {
        self.channels.write().await.remove(&channel_id);
    }

    async fn add(&self, channel_id: NonZeroU64, params: TrackEntryParams) {
        self.channels.write().await.insert(channel_id, params);
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

        let now = OffsetDateTime::now_utc();
        self.store_last_pp(pp, now);
        let upsert_fut = Context::psql().upsert_tracked_last_pp(user_id, mode, pp, now);

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

    fn store_last_pp(&self, pp: f32, datetime: OffsetDateTime) {
        self.last_pp.store(pp.to_bits(), Ordering::SeqCst);
        self.last_updated
            .store(datetime.unix_timestamp(), Ordering::SeqCst);
    }

    async fn insert(&self, user: DbTrackedOsuUser) {
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

        self.channels.write().await.insert(channel_id, params);
    }
}

#[derive(Clone, Default)]
pub struct TrackedUser {
    modes: [Arc<TrackEntry>; 4],
}

impl TrackedUser {
    pub async fn get(&self, mode: GameMode) -> Option<Arc<TrackEntry>> {
        let entry = &self.modes[mode as usize];

        entry.is_empty().await.not().then(|| Arc::clone(entry))
    }

    /// Same as [`TrackedUser::get`] but does *not* perform emptyness check
    pub fn get_unchecked(&self, mode: GameMode) -> Arc<TrackEntry> {
        Arc::clone(&self.modes[mode as usize])
    }

    pub async fn is_empty(&self) -> bool {
        for entry in self.modes.iter() {
            if !entry.is_empty().await {
                return false;
            }
        }

        true
    }

    pub async fn remove_channel(&self, channel_id: NonZeroU64, mode: Option<GameMode>) {
        if let Some(mode) = mode {
            if let Some(entry) = self.get(mode).await {
                entry.remove_channel(channel_id).await;
            }
        } else {
            for entry in self.modes.iter() {
                entry.remove_channel(channel_id).await;
            }
        }
    }

    pub async fn add(&self, mode: GameMode, channel_id: NonZeroU64, params: TrackEntryParams) {
        self.modes[mode as usize].add(channel_id, params).await;
    }

    pub fn needs_last_pp(&self, mode: GameMode) -> bool {
        self.modes[mode as usize].needs_last_pp()
    }

    pub async fn insert_last_pp(&self, user_id: u32, mode: GameMode, top_scores: &[Score]) {
        self.modes[mode as usize]
            .insert_last_pp(user_id, mode, top_scores)
            .await
    }

    pub async fn insert(&self, user: DbTrackedOsuUser) {
        self.modes[user.gamemode as usize].insert(user).await;
    }
}
