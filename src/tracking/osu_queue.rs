use std::{
    cmp::Reverse,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration as StdDuration,
};

use ::time::{Duration, OffsetDateTime};
use flexmap::tokio::TokioMutexMap;
use futures::StreamExt;
use hashbrown::hash_map::{DefaultHashBuilder, Entry, HashMap};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use priority_queue::PriorityQueue;
use rosu_v2::model::GameMode;
use tokio::{sync::Mutex, time};
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{database::TrackingUser, BotResult, Database};

pub use super::{osu_tracking_loop, process_osu_tracking};

const SECOND: StdDuration = StdDuration::from_secs(1);

static OSU_TRACKING_INTERVAL: OnceCell<Duration> = OnceCell::with_value(Duration::minutes(150));

pub fn default_tracking_interval() -> Duration {
    unsafe { *OSU_TRACKING_INTERVAL.get_unchecked() }
}

type TrackingQueue =
    Mutex<PriorityQueue<TrackingEntry, Reverse<OffsetDateTime>, DefaultHashBuilder>>;

type Channels = HashMap<Id<ChannelMarker>, usize>;

pub struct TrackingStats {
    pub next_pop: TrackingEntry,
    pub users: usize,
    pub queue: usize,
    pub last_pop: OffsetDateTime,
    pub interval: i64,
    pub tracking: bool,
    pub wait_interval: i64,
    pub ms_per_track: i64,
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub struct TrackingEntry {
    pub user_id: u32,
    pub mode: GameMode,
}

impl From<&TrackingUser> for TrackingEntry {
    #[inline]
    fn from(user: &TrackingUser) -> Self {
        Self {
            user_id: user.user_id,
            mode: user.mode,
        }
    }
}

pub struct OsuTracking {
    queue: OsuTrackingQueue,
}

impl OsuTracking {
    #[cold]
    pub async fn new(psql: &Database) -> BotResult<Self> {
        OsuTrackingQueue::new(psql)
            .await
            .map(|queue| Self { queue })
    }

    pub fn set_tracking(&self, value: bool) {
        self.queue.stop_tracking.store(value, Ordering::SeqCst);
    }

    pub fn toggle_tracking(&self) {
        self.queue.stop_tracking.fetch_nand(true, Ordering::SeqCst);
    }

    pub fn stop_tracking(&self) -> bool {
        self.queue.stop_tracking.load(Ordering::Acquire)
    }

    pub fn set_interval(&self, duration: Duration) {
        *self.queue.interval.write() = duration;
    }

    pub fn interval(&self) -> Duration {
        *self.queue.interval.read()
    }

    pub async fn reset(&self, user_id: u32, mode: GameMode) {
        self.queue.reset(user_id, mode).await;
    }

    pub async fn update_last_date(
        &self,
        user_id: u32,
        mode: GameMode,
        new_date: OffsetDateTime,
        psql: &Database,
    ) -> BotResult<()> {
        if self.queue.update_last_date(user_id, mode, new_date).await {
            let entry = TrackingEntry { user_id, mode };
            psql.update_osu_tracking_date(&entry, new_date).await?;
        }

        Ok(())
    }

    pub async fn get_tracked(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Option<(OffsetDateTime, Channels)> {
        self.queue.get_tracked(user_id, mode).await
    }

    pub async fn pop(&self) -> Option<(TrackingEntry, usize)> {
        self.queue.pop().await
    }

    pub async fn remove_user_all(&self, user_id: u32, psql: &Database) -> BotResult<()> {
        for mode in self.queue.remove_user_all(user_id).await {
            psql.remove_osu_tracking(user_id, mode).await?;
        }

        Ok(())
    }

    pub async fn remove_user(
        &self,
        user_id: u32,
        mode: Option<GameMode>,
        channel: Id<ChannelMarker>,
        psql: &Database,
    ) -> BotResult<()> {
        let remove_entries = self.queue.remove_user(user_id, mode, channel).await;
        self.remove(remove_entries, psql).await?;

        Ok(())
    }

    pub async fn remove_channel(
        &self,
        channel: Id<ChannelMarker>,
        mode: Option<GameMode>,
        psql: &Database,
    ) -> BotResult<usize> {
        let remove_entries = self.queue.remove_channel(channel, mode).await;
        let len = remove_entries.len();
        self.remove(remove_entries, psql).await?;

        Ok(len)
    }

    async fn remove(&self, remove: Vec<RemoveEntry>, psql: &Database) -> BotResult<()> {
        for remove_entry in remove {
            let TrackingEntry { user_id, mode } = remove_entry.entry;

            if remove_entry.no_longer_tracked {
                psql.remove_osu_tracking(user_id, mode).await?;
            } else {
                let guard = self.queue.users.lock(&remove_entry.entry).await;

                if let Some(user) = guard.get() {
                    psql.update_osu_tracking(user_id, mode, user.last_top_score, &user.channels)
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
        limit: usize,
        psql: &Database,
    ) -> BotResult<bool> {
        let added = self
            .queue
            .add(user_id, mode, last_top_score, channel, limit)
            .await;

        match added {
            AddEntry::AddedNew => {
                psql.insert_osu_tracking(user_id, mode, last_top_score, channel, limit)
                    .await?;
            }
            AddEntry::NotAdded => return Ok(false),
            AddEntry::Added | AddEntry::UpdatedLimit => {
                let entry = TrackingEntry { user_id, mode };
                let guard = self.queue.users.lock(&entry).await;

                if let Some(user) = guard.get() {
                    psql.update_osu_tracking(user_id, mode, user.last_top_score, &user.channels)
                        .await?;
                } else {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    pub async fn list(&self, channel: Id<ChannelMarker>) -> Vec<(u32, GameMode, usize)> {
        self.queue.list(channel).await
    }

    pub async fn stats(&self) -> TrackingStats {
        self.queue.stats().await
    }
}

pub struct OsuTrackingQueue {
    queue: TrackingQueue,
    users: TokioMutexMap<TrackingEntry, TrackingUser>,
    last_date: Mutex<OffsetDateTime>,
    pub interval: RwLock<Duration>,
    pub stop_tracking: AtomicBool,
}

impl OsuTrackingQueue {
    #[cold]
    async fn new(psql: &Database) -> BotResult<Self> {
        let users = psql.get_osu_trackings().await?;

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
    async fn reset(&self, user_id: u32, mode: GameMode) {
        let now = OffsetDateTime::now_utc();
        *self.last_date.lock().await = now;
        let entry = TrackingEntry { user_id, mode };
        self.queue.lock().await.push_decrease(entry, Reverse(now));
    }

    /// Returns whether the entry was updated
    /// i.e. if `new_date` comes after the latest top play of the user
    async fn update_last_date(
        &self,
        user_id: u32,
        mode: GameMode,
        new_date: OffsetDateTime,
    ) -> bool {
        self.users
            .lock(&TrackingEntry { user_id, mode })
            .await
            .get_mut()
            .filter(|user| new_date > user.last_top_score)
            .map_or(false, |mut user| {
                user.last_top_score = new_date;

                true
            })
    }

    /// Returns all channels in which a user is tracked for a mode
    /// and also the date time of the user's last top score
    async fn get_tracked(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Option<(OffsetDateTime, Channels)> {
        self.users
            .lock(&TrackingEntry { user_id, mode })
            .await
            .get()
            .map(|user| (user.last_top_score, user.channels.to_owned()))
    }

    /// Pop a user from the queue to be checked for tracking
    async fn pop(&self) -> Option<(TrackingEntry, usize)> {
        let len = self.queue.lock().await.len();

        if len == 0 || self.stop_tracking.load(Ordering::Acquire) {
            time::sleep(StdDuration::from_secs(5)).await;

            return None;
        }

        let last_date = *self.last_date.lock().await;
        let interval = last_date + *self.interval.read() - OffsetDateTime::now_utc();
        let ms_per_track = interval.whole_milliseconds() as f32 / len as f32;
        time::sleep(StdDuration::from_millis(ms_per_track as u64)).await;

        // Pop user and return them
        let mut queue = self.queue.lock().await;

        loop {
            let entry = queue.pop().map(|(entry, _)| entry)?;

            let guard = match time::timeout(SECOND, self.users.lock(&entry)).await {
                Ok(guard) => guard,
                Err(_) => {
                    warn!("Timed out while trying to pop user");
                    queue.push(entry, Reverse(OffsetDateTime::now_utc()));

                    continue;
                }
            };

            if let Some(amount) = guard.get().and_then(|u| u.channels.values().max().copied()) {
                return Some((entry, amount));
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
            let entry = TrackingEntry { user_id, mode };

            self.queue.lock().await.remove(&entry);
            self.users.lock(&entry).await.remove();
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
                && guard.value_mut().remove_channel(channel)
            {
                removed.push(RemoveEntry::from(guard.key()));
            }
        }

        for user_remove in removed.iter_mut() {
            let mut guard = self.users.own(user_remove.entry).await;

            if let Entry::Occupied(entry) = guard.entry() {
                if entry.get().channels.is_empty() {
                    user_remove.no_longer_tracked = true;
                    entry.remove();
                    self.queue.lock().await.remove(&user_remove.entry);
                }
            }
        }

        removed
    }

    /// Return all entries which were tracked in the channel and whether they are no longer tracked
    /// anywhere
    async fn remove_channel(
        &self,
        channel: Id<ChannelMarker>,
        mode: Option<GameMode>,
    ) -> Vec<RemoveEntry> {
        let mut removed = Vec::new();
        let mut stream = self.users.iter_mut();

        while let Some(mut guard) = stream.next().await {
            if mode.map_or(true, |m| guard.key().mode == m)
                && guard.value_mut().remove_channel(channel)
            {
                removed.push(RemoveEntry::from(guard.key()));
            }
        }

        for channel_remove in removed.iter_mut() {
            let mut guard = self.users.own(channel_remove.entry).await;

            if let Entry::Occupied(entry) = guard.entry() {
                if entry.get().channels.is_empty() {
                    channel_remove.no_longer_tracked = true;
                    entry.remove();
                    self.queue.lock().await.remove(&channel_remove.entry);
                }
            }
        }

        removed
    }

    /// Returns whether the entry has been newly added, updated, or not added at all
    async fn add(
        &self,
        user_id: u32,
        mode: GameMode,
        last_top_score: OffsetDateTime,
        channel: Id<ChannelMarker>,
        limit: usize,
    ) -> AddEntry {
        let key = TrackingEntry { user_id, mode };
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
                let tracking_user =
                    TrackingUser::new(user_id, mode, last_top_score, channel, limit);

                entry.insert(tracking_user);

                let now = OffsetDateTime::now_utc();
                *self.last_date.lock().await = now;
                let entry = TrackingEntry { user_id, mode };
                self.queue.lock().await.push(entry, Reverse(now));

                AddEntry::AddedNew
            }
        }
    }

    /// Returns all entries that are tracked in the channel
    async fn list(&self, channel: Id<ChannelMarker>) -> Vec<(u32, GameMode, usize)> {
        self.users
            .iter()
            .filter_map(|guard| {
                let limit = match guard.value().channels.get(&channel) {
                    Some(limit) => *limit,
                    None => return futures::future::ready(None),
                };

                let TrackingEntry { user_id, mode } = guard.key();

                futures::future::ready(Some((*user_id, *mode, limit)))
            })
            .collect()
            .await
    }

    async fn stats(&self) -> TrackingStats {
        let (next_pop, queue) = {
            let guard = self.queue.lock().await;

            (guard.peek().map(|(&key, _)| key).unwrap(), guard.len())
        };

        let users = self.users.len().await;
        let last_pop = *self.last_date.lock().await;
        let interval = *self.interval.read();
        let tracking = !self.stop_tracking.load(Ordering::Acquire);

        let wait_interval = (last_pop + interval - OffsetDateTime::now_utc()).whole_seconds();
        let ms_per_track = wait_interval as f32 / queue as f32;

        TrackingStats {
            next_pop,
            users,
            queue,
            last_pop,
            interval: interval.whole_seconds(),
            tracking,
            wait_interval,
            ms_per_track: ms_per_track as i64,
        }
    }
}

pub struct RemoveEntry {
    entry: TrackingEntry,
    no_longer_tracked: bool,
}

impl From<&TrackingEntry> for RemoveEntry {
    #[inline]
    fn from(entry: &TrackingEntry) -> Self {
        Self {
            entry: *entry,
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
