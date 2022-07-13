use std::{
    cmp::Reverse,
    sync::atomic::{AtomicBool, Ordering},
};

use ::time::{Duration, OffsetDateTime};
use dashmap::DashMap;
use hashbrown::hash_map::{DefaultHashBuilder, HashMap};
use once_cell::sync::OnceCell;
use parking_lot::{Mutex, RwLock};
use priority_queue::PriorityQueue;
use rosu_v2::model::GameMode;
use smallvec::SmallVec;
use tokio::time;
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{database::TrackingUser, BotResult, Database};

pub use super::{osu_tracking_loop, process_osu_tracking};

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
    fn from(user: &TrackingUser) -> Self {
        Self {
            user_id: user.user_id,
            mode: user.mode,
        }
    }
}

pub struct OsuTracking {
    queue: TrackingQueue,
    users: DashMap<TrackingEntry, TrackingUser>,
    last_date: Mutex<OffsetDateTime>,
    pub interval: RwLock<Duration>,
    pub stop_tracking: AtomicBool,
}

impl OsuTracking {
    #[cold]
    pub async fn new(psql: &Database) -> BotResult<Self> {
        let users = psql.get_osu_trackings().await?;

        let queue = users
            .iter()
            .map(|guard| (*guard.key(), Reverse(OffsetDateTime::now_utc())))
            .collect();

        Ok(Self {
            queue: Mutex::new(queue),
            users,
            last_date: Mutex::new(OffsetDateTime::now_utc()),
            interval: RwLock::new(default_tracking_interval()),
            stop_tracking: AtomicBool::new(false),
        })
    }

    pub fn stats(&self) -> TrackingStats {
        let next_pop = self.queue.lock().peek().map(|(&key, _)| key).unwrap();
        let users = self.users.len();
        let queue = self.queue.lock().len();
        let last_pop = *self.last_date.lock();
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

    pub fn reset(&self, user_id: u32, mode: GameMode) {
        let now = OffsetDateTime::now_utc();
        *self.last_date.lock() = now;
        let entry = TrackingEntry { user_id, mode };
        self.queue.lock().push_decrease(entry, Reverse(now));
    }

    pub async fn update_last_date(
        &self,
        user_id: u32,
        mode: GameMode,
        new_date: OffsetDateTime,
        psql: &Database,
    ) -> BotResult<()> {
        let entry = TrackingEntry { user_id, mode };

        let update = self
            .users
            .get_mut(&entry)
            .filter(|user| new_date > user.last_top_score)
            .map_or(false, |mut user| {
                user.last_top_score = new_date;

                true
            });

        if update {
            psql.update_osu_tracking_date(&entry, new_date).await?;
        }

        Ok(())
    }

    pub fn get_tracked(&self, user_id: u32, mode: GameMode) -> Option<(OffsetDateTime, Channels)> {
        let entry = TrackingEntry { user_id, mode };

        self.users
            .get(&entry)
            .map(|user| (user.last_top_score, user.channels.to_owned()))
    }

    pub async fn pop(&self) -> Option<(TrackingEntry, usize)> {
        let len = self.queue.lock().len();

        if len == 0 || self.stop_tracking.load(Ordering::Acquire) {
            time::sleep(time::Duration::from_secs(5)).await;

            return None;
        }

        let last_date = *self.last_date.lock();
        let interval = last_date + *self.interval.read() - OffsetDateTime::now_utc();
        let ms_per_track = interval.whole_milliseconds() as f32 / len as f32;
        time::sleep(time::Duration::from_millis(ms_per_track as u64)).await;

        // Pop user and return them
        let mut queue = self.queue.lock();

        loop {
            let entry = queue.pop().map(|(entry, _)| entry)?;
            let guard = self.users.get(&entry);

            if let Some(amount) = guard.and_then(|g| g.value().channels.values().max().copied()) {
                return Some((entry, amount));
            }
        }
    }

    pub async fn remove_user_all(&self, user_id: u32, psql: &Database) -> BotResult<()> {
        let removed: SmallVec<[_; 4]> = self
            .users
            .iter()
            .filter(|guard| guard.key().user_id == user_id)
            .map(|guard| guard.key().mode)
            .collect();

        for mode in removed {
            let entry = TrackingEntry { user_id, mode };

            psql.remove_osu_tracking(user_id, mode).await?;
            self.queue.lock().remove(&entry);
            self.users.remove(&entry);
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
        let removed: SmallVec<[_; 4]> = self
            .users
            .iter_mut()
            .filter(|guard| {
                let key = guard.key();

                key.user_id == user_id && mode.map_or(true, |m| key.mode == m)
            })
            .filter_map(|mut guard| {
                guard
                    .value_mut()
                    .remove_channel(channel)
                    .then(|| guard.key().mode)
            })
            .collect();

        for mode in removed {
            let key = TrackingEntry { user_id, mode };
            let entry = self.users.get(&key);

            match entry.map(|guard| guard.value().channels.is_empty()) {
                Some(true) => {
                    psql.remove_osu_tracking(user_id, mode).await?;
                    self.queue.lock().remove(&key);
                    self.users.remove(&key);
                }
                Some(false) => {
                    if let Some(guard) = self.users.get(&key) {
                        let user = guard.value();

                        psql.update_osu_tracking(user_id, mode, user.last_top_score, &user.channels)
                            .await?
                    }
                }
                None => warn!("Should not be reachable"),
            }
        }

        Ok(())
    }

    pub async fn remove_channel(
        &self,
        channel: Id<ChannelMarker>,
        mode: Option<GameMode>,
        psql: &Database,
    ) -> BotResult<usize> {
        let mut count = 0;

        let to_remove: Vec<_> = self
            .users
            .iter_mut()
            .filter(|guard| match mode {
                Some(mode) => guard.key().mode == mode,
                None => true,
            })
            .filter_map(|mut guard| {
                guard
                    .value_mut()
                    .remove_channel(channel)
                    .then(|| *guard.key())
            })
            .collect();

        for key in to_remove {
            let is_empty = match self.users.get(&key) {
                Some(guard) => guard.value().channels.is_empty(),
                None => continue,
            };

            if is_empty {
                // tracking_debug!("Removing {:?} from tracking (all)", key);
                psql.remove_osu_tracking(key.user_id, key.mode).await?;
                self.queue.lock().remove(&key);
                self.users.remove(&key);
            } else {
                let guard = match self.users.get(&key) {
                    Some(guard) => guard,
                    None => continue,
                };

                let user = guard.value();
                let TrackingEntry { user_id, mode } = key;

                psql.update_osu_tracking(user_id, mode, user.last_top_score, &user.channels)
                    .await?
            }

            count += 1;
        }

        Ok(count)
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
        let key = TrackingEntry { user_id, mode };

        match self.users.get_mut(&key) {
            Some(mut guard) => match guard.value().channels.get(&channel) {
                Some(old_limit) => match *old_limit == limit {
                    true => return Ok(false),
                    false => {
                        let value = guard.value_mut();
                        value.channels.insert(channel, limit);

                        psql.update_osu_tracking(
                            user_id,
                            mode,
                            value.last_top_score,
                            &value.channels,
                        )
                        .await?;
                    }
                },
                None => {
                    let value = guard.value_mut();
                    value.channels.insert(channel, limit);

                    psql.update_osu_tracking(user_id, mode, value.last_top_score, &value.channels)
                        .await?;
                }
            },
            None => {
                psql.insert_osu_tracking(user_id, mode, last_top_score, channel, limit)
                    .await?;

                let tracking_user =
                    TrackingUser::new(user_id, mode, last_top_score, channel, limit);

                self.users.insert(key, tracking_user);
                let now = OffsetDateTime::now_utc();
                *self.last_date.lock() = now;
                let entry = TrackingEntry { user_id, mode };
                self.queue.lock().push(entry, Reverse(now));
            }
        }

        Ok(true)
    }

    pub fn list(&self, channel: Id<ChannelMarker>) -> Vec<(u32, GameMode, usize)> {
        self.users
            .iter()
            .filter_map(|guard| {
                let limit = *guard.value().channels.get(&channel)?;
                let TrackingEntry { user_id, mode } = guard.key();

                Some((*user_id, *mode, limit))
            })
            .collect()
    }
}
