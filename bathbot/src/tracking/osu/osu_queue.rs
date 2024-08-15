use std::{
    cmp::Reverse,
    collections::HashMap as StdHashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        RwLock,
    },
    time::Duration as StdDuration,
};

use ::time::{Duration, OffsetDateTime};
use bathbot_psql::{
    model::osu::{TrackedOsuUserKey, TrackedOsuUserValue},
    Database,
};
use bathbot_util::IntHasher;
use eyre::Result;
use flexmap::tokio::TokioMutexMap;
use futures::{future, StreamExt};
use hashbrown::hash_map::{DefaultHashBuilder, Entry};
use once_cell::sync::OnceCell;
use priority_queue::PriorityQueue;
use rosu_v2::model::GameMode;
use tokio::{sync::Mutex, time};
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{core::Context, manager::OsuTrackingManager};

static OSU_TRACKING_INTERVAL: OnceCell<Duration> = OnceCell::with_value(Duration::minutes(210));

pub fn default_tracking_interval() -> Duration {
    unsafe { *OSU_TRACKING_INTERVAL.get_unchecked() }
}

type TrackingQueue =
    Mutex<PriorityQueue<TrackedOsuUserKey, Reverse<OffsetDateTime>, DefaultHashBuilder>>;

pub struct TrackingStats {
    pub next_pop: Option<TrackedOsuUserKey>,
    pub users: usize,
    pub queue: usize,
    pub last_pop: OffsetDateTime,
    pub interval: i64,
    pub tracking: bool,
    pub wait_interval: i64,
    pub ms_per_track: i64,
}

pub struct OsuTracking {
    queue: OsuTrackingQueue,
}

impl OsuTracking {
    // This is called before the global context is set so we need to pass a DB
    // reference here.
    #[cold]
    pub async fn new(psql: &Database) -> Result<Self> {
        OsuTrackingQueue::new(psql)
            .await
            .map(|queue| Self { queue })
    }

    pub fn set_stop_tracking(&self, value: bool) {
        self.queue.stop_tracking.store(value, Ordering::SeqCst);
    }

    pub fn toggle_tracking(&self) {
        self.queue.stop_tracking.fetch_nand(true, Ordering::SeqCst);
    }

    pub fn stop_tracking(&self) -> bool {
        self.queue.stop_tracking.load(Ordering::Acquire)
    }

    pub fn set_interval(&self, duration: Duration) {
        *self.queue.interval.write().unwrap() = duration;
    }

    pub fn interval(&self) -> Duration {
        *self.queue.interval.read().unwrap()
    }

    pub async fn reset(&self, key: TrackedOsuUserKey) {
        self.queue.reset(key).await;
    }

    pub async fn update_last_date(
        &self,
        key: TrackedOsuUserKey,
        new_date: OffsetDateTime,
    ) -> Result<()> {
        if self.queue.update_last_date(key, new_date).await {
            Context::osu_tracking().update_date(key).await?;
        }

        Ok(())
    }

    pub async fn get_tracked(
        &self,
        key: TrackedOsuUserKey,
    ) -> Option<TrackedOsuUserValue<IntHasher>> {
        self.queue.get_tracked(key).await
    }

    pub async fn pop(&self) -> Option<(TrackedOsuUserKey, u8)> {
        self.queue.pop().await
    }

    pub async fn remove_user_all(&self, user_id: u32) -> Result<()> {
        let manager = Context::osu_tracking();

        for mode in self.queue.remove_user_all(user_id).await {
            let key = TrackedOsuUserKey { user_id, mode };
            manager.remove_user(key).await?;
        }

        Ok(())
    }

    pub async fn remove_user(
        &self,
        user_id: u32,
        mode: Option<GameMode>,
        channel: Id<ChannelMarker>,
    ) -> Result<()> {
        let remove_entries = self.queue.remove_user(user_id, mode, channel).await;
        self.remove(remove_entries).await?;

        Ok(())
    }

    pub async fn remove_channel(
        &self,
        channel: Id<ChannelMarker>,
        mode: Option<GameMode>,
    ) -> Result<usize> {
        let remove_entries = self.queue.remove_channel(channel, mode).await;
        let len = remove_entries.len();
        self.remove(remove_entries).await?;

        Ok(len)
    }

    async fn remove(&self, remove: Vec<RemoveEntry>) -> Result<()> {
        let manager = Context::osu_tracking();

        for remove_entry in remove {
            if remove_entry.no_longer_tracked {
                manager.remove_user(remove_entry.key).await?;
            } else {
                let guard = self.queue.users.lock(&remove_entry.key).await;

                if let Some(user) = guard.get() {
                    manager
                        .update_channels(remove_entry.key, &user.channels)
                        .await?;
                }
            }
        }

        Ok(())
    }

    pub async fn add(
        &self,
        user_id: u32,
        mode: GameMode,
        last_top_score: OffsetDateTime,
        channel: Id<ChannelMarker>,
        limit: u8,
    ) -> Result<bool> {
        let manager = Context::osu_tracking();
        let key = TrackedOsuUserKey { user_id, mode };
        let added = self.queue.add(key, last_top_score, channel, limit).await;

        match added {
            AddEntry::AddedNew => manager.insert_user(key, channel, limit).await?,
            AddEntry::NotAdded => return Ok(false),
            AddEntry::Added | AddEntry::UpdatedLimit => {
                let guard = self.queue.users.lock(&key).await;

                if let Some(user) = guard.get() {
                    manager.update_channels(key, &user.channels).await?;
                } else {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    pub async fn list(&self, channel: Id<ChannelMarker>) -> Vec<(TrackedOsuUserKey, u8)> {
        self.queue.list(channel).await
    }

    pub async fn stats(&self) -> TrackingStats {
        self.queue.stats().await
    }
}

pub struct OsuTrackingQueue {
    queue: TrackingQueue,
    users: TokioMutexMap<TrackedOsuUserKey, TrackedOsuUserValue<IntHasher>>,
    last_date: Mutex<OffsetDateTime>,
    pub interval: RwLock<Duration>,
    pub stop_tracking: AtomicBool,
}

impl OsuTrackingQueue {
    // This is called before the global context is set so we need to pass a DB
    // reference here.
    #[cold]
    async fn new(psql: &Database) -> Result<Self> {
        let users = OsuTrackingManager::new(psql).get_users().await?;
        let now = OffsetDateTime::now_utc();

        let queue = users
            .iter()
            .map(|(entry, _)| (*entry, Reverse(now)))
            .collect();

        let users = users.into_iter().collect();

        Ok(Self {
            queue: Mutex::new(queue),
            users,
            last_date: Mutex::new(now),
            interval: RwLock::new(default_tracking_interval()),
            stop_tracking: AtomicBool::new(false),
        })
    }

    /// Put the entry at the end of the queue
    async fn reset(&self, key: TrackedOsuUserKey) {
        let now = OffsetDateTime::now_utc();
        *self.last_date.lock().await = now;
        self.queue.lock().await.push_decrease(key, Reverse(now));
    }

    /// Returns whether the entry was updated
    /// i.e. if `new_date` comes after the latest top play of the user
    async fn update_last_date(&self, key: TrackedOsuUserKey, new_date: OffsetDateTime) -> bool {
        self.users
            .lock(&key)
            .await
            .get_mut()
            .filter(|value| new_date > value.last_update)
            .map_or(false, |value| {
                value.last_update = new_date;

                true
            })
    }

    /// Returns all channels in which a user is tracked for a mode
    /// and also the date time of the user's last top score
    async fn get_tracked(&self, key: TrackedOsuUserKey) -> Option<TrackedOsuUserValue<IntHasher>> {
        self.users
            .lock(&key)
            .await
            .get()
            .map(TrackedOsuUserValue::to_owned)
    }

    /// Pop a user from the queue to be checked for tracking
    async fn pop(&self) -> Option<(TrackedOsuUserKey, u8)> {
        let len = self.queue.lock().await.len();

        if len == 0 || self.stop_tracking.load(Ordering::Acquire) {
            time::sleep(StdDuration::from_secs(5)).await;

            return None;
        }

        let last_date = *self.last_date.lock().await;
        let interval = last_date + *self.interval.read().unwrap() - OffsetDateTime::now_utc();
        let ms_per_track = interval.whole_milliseconds() as f32 / len as f32;
        time::sleep(StdDuration::from_millis(ms_per_track as u64)).await;

        // Pop user and return them
        loop {
            let key = self.queue.lock().await.pop().map(|(key, _)| key)?;
            let guard = self.users.lock(&key).await;

            if let Some(amount) = guard.get().and_then(|u| u.channels.values().max().copied()) {
                return Some((key, amount));
            }
        }
    }

    /// Returns all game modes for which the user was tracked in some channel
    async fn remove_user_all(&self, user_id: u32) -> Vec<GameMode> {
        let mut to_remove = Vec::with_capacity(2);
        let mut stream = self.users.iter();

        while let Some(guard) = stream.next().await {
            if guard.key().user_id == user_id {
                to_remove.push(guard.key().mode);
            }
        }

        for &mode in to_remove.iter() {
            let key = TrackedOsuUserKey { user_id, mode };

            self.queue.lock().await.remove(&key);
            self.users.lock(&key).await.remove();
        }

        to_remove
    }

    /// Returns all game modes for which the user was tracked the channel
    async fn remove_user(
        &self,
        user_id: u32,
        mode: Option<GameMode>,
        channel: Id<ChannelMarker>,
    ) -> Vec<RemoveEntry> {
        let mut removed = Vec::with_capacity(2);
        let mut stream = self.users.iter_mut();

        while let Some(mut guard) = stream.next().await {
            if guard.key().user_id == user_id
                && mode.map_or(true, |m| guard.key().mode == m)
                && guard
                    .value_mut()
                    .channels
                    .remove(&channel.into_nonzero())
                    .is_some()
            {
                removed.push(RemoveEntry::from(guard.key()));
            }
        }

        for user_remove in removed.iter_mut() {
            let mut guard = self.users.own(user_remove.key).await;

            if let Entry::Occupied(entry) = guard.entry() {
                if entry.get().channels.is_empty() {
                    user_remove.no_longer_tracked = true;
                    entry.remove();
                    self.queue.lock().await.remove(&user_remove.key);
                }
            }
        }

        removed
    }

    /// Return all entries which were tracked in the channel and whether they
    /// are no longer tracked anywhere
    async fn remove_channel(
        &self,
        channel: Id<ChannelMarker>,
        mode: Option<GameMode>,
    ) -> Vec<RemoveEntry> {
        let mut removed = Vec::new();
        let mut stream = self.users.iter_mut();

        while let Some(mut guard) = stream.next().await {
            if mode.map_or(true, |m| guard.key().mode == m)
                && guard
                    .value_mut()
                    .channels
                    .remove(&channel.into_nonzero())
                    .is_some()
            {
                removed.push(RemoveEntry::from(guard.key()));
            }
        }

        for channel_remove in removed.iter_mut() {
            let mut guard = self.users.own(channel_remove.key).await;

            if let Entry::Occupied(entry) = guard.entry() {
                if entry.get().channels.is_empty() {
                    channel_remove.no_longer_tracked = true;
                    entry.remove();
                    self.queue.lock().await.remove(&channel_remove.key);
                }
            }
        }

        removed
    }

    /// Returns whether the entry has been newly added, updated, or not added at
    /// all
    async fn add(
        &self,
        key: TrackedOsuUserKey,
        last_top_score: OffsetDateTime,
        channel: Id<ChannelMarker>,
        limit: u8,
    ) -> AddEntry {
        let channel = channel.into_nonzero();
        let mut guard = self.users.own(key).await;

        match guard.entry() {
            Entry::Occupied(mut entry) => match entry.get().channels.get(&channel) {
                Some(old_limit) => match *old_limit == limit {
                    true => AddEntry::NotAdded,
                    false => {
                        entry.get_mut().channels.insert(channel, limit);

                        AddEntry::UpdatedLimit
                    }
                },
                None => {
                    entry.get_mut().channels.insert(channel, limit);

                    AddEntry::Added
                }
            },
            Entry::Vacant(entry) => {
                let mut channels = StdHashMap::default();
                channels.insert(channel, limit);

                let value = TrackedOsuUserValue {
                    channels,
                    last_update: last_top_score,
                };

                entry.insert(value);

                let now = OffsetDateTime::now_utc();
                *self.last_date.lock().await = now;
                self.queue.lock().await.push(key, Reverse(now));

                AddEntry::AddedNew
            }
        }
    }

    /// Returns all entries that are tracked in the channel
    async fn list(&self, channel: Id<ChannelMarker>) -> Vec<(TrackedOsuUserKey, u8)> {
        self.users
            .iter()
            .filter_map(
                |guard| match guard.value().channels.get(&channel.into_nonzero()) {
                    Some(limit) => future::ready(Some((*guard.key(), *limit))),
                    None => future::ready(None),
                },
            )
            .collect()
            .await
    }

    async fn stats(&self) -> TrackingStats {
        let (next_pop, queue) = {
            let guard = self.queue.lock().await;

            (guard.peek().map(|(&key, _)| key), guard.len())
        };

        let users = self.users.len().await;
        let last_pop = *self.last_date.lock().await;
        let interval = *self.interval.read().unwrap();
        let tracking = !self.stop_tracking.load(Ordering::Acquire);

        let wait_interval = last_pop + interval - OffsetDateTime::now_utc();
        let ms_per_track = wait_interval.whole_milliseconds() as f32 / queue as f32;

        TrackingStats {
            next_pop,
            users,
            queue,
            last_pop,
            interval: interval.whole_seconds(),
            tracking,
            wait_interval: wait_interval.whole_seconds(),
            ms_per_track: ms_per_track as i64,
        }
    }
}

pub struct RemoveEntry {
    key: TrackedOsuUserKey,
    no_longer_tracked: bool,
}

impl From<&TrackedOsuUserKey> for RemoveEntry {
    #[inline]
    fn from(key: &TrackedOsuUserKey) -> Self {
        Self {
            key: *key,
            no_longer_tracked: false,
        }
    }
}

pub enum AddEntry {
    AddedNew,
    Added,
    NotAdded,
    UpdatedLimit,
}
