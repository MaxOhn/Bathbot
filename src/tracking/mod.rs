mod tracking_loop;

use std::{
    cmp::Reverse,
    sync::atomic::{AtomicBool, Ordering},
};

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use hashbrown::hash_map::{DefaultHashBuilder, HashMap};
use parking_lot::RwLock;
use priority_queue::PriorityQueue;
use rosu_v2::model::GameMode;
use smallvec::SmallVec;
use tokio::time;
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::{database::TrackingUser, BotResult, Database};

pub use self::tracking_loop::{process_tracking, tracking_loop};

lazy_static::lazy_static! {
    pub static ref OSU_TRACKING_INTERVAL: Duration = Duration::minutes(120);
    pub static ref OSU_TRACKING_COOLDOWN: f32 = 5000.0; // ms
}

type TrackingQueue =
    RwLock<PriorityQueue<TrackingEntry, Reverse<DateTime<Utc>>, DefaultHashBuilder>>;

type Channels = HashMap<Id<ChannelMarker>, usize>;

pub struct TrackingStats {
    pub next_pop: TrackingEntry,
    pub users: usize,
    pub queue: usize,
    pub last_pop: DateTime<Utc>,
    pub interval: i64,
    pub cooldown: i64,
    pub tracking: bool,
    pub wait_interval: i64,
    pub ms_per_track: i64,
    pub amount: usize,
    pub delay: u64,
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
    last_date: RwLock<DateTime<Utc>>,
    cooldown: RwLock<f32>,
    pub interval: RwLock<Duration>,
    pub stop_tracking: AtomicBool,
}

impl OsuTracking {
    #[cold]
    pub async fn new(psql: &Database) -> BotResult<Self> {
        let users = psql.get_osu_trackings().await?;

        let queue = users
            .iter()
            .map(|guard| (*guard.key(), Reverse(Utc::now())))
            .collect();

        Ok(Self {
            queue: RwLock::new(queue),
            users,
            last_date: RwLock::new(Utc::now()),
            cooldown: RwLock::new(*OSU_TRACKING_COOLDOWN),
            interval: RwLock::new(*OSU_TRACKING_INTERVAL),
            stop_tracking: AtomicBool::new(false),
        })
    }

    pub fn stats(&self) -> TrackingStats {
        let next_pop = self.queue.read().peek().map(|(&key, _)| key).unwrap();
        let users = self.users.len();
        let queue = self.queue.read().len();
        let last_pop = *self.last_date.read();
        let interval = *self.interval.read();
        let cooldown = *self.cooldown.read();
        let tracking = !self.stop_tracking.load(Ordering::Acquire);

        let wait_interval = (last_pop + interval - Utc::now()).num_milliseconds();
        let ms_per_track = wait_interval as f32 / queue as f32;
        let amount = (cooldown / ms_per_track).max(1.0);
        let delay = (ms_per_track * amount) as u64;

        TrackingStats {
            next_pop,
            users,
            queue,
            last_pop,
            interval: interval.num_seconds(),
            cooldown: cooldown as i64,
            tracking,
            wait_interval: wait_interval / 1000,
            ms_per_track: ms_per_track as i64,
            amount: amount as usize,
            delay,
        }
    }

    // ms
    #[inline]
    pub fn set_cooldown(&self, new_cooldown: f32) -> f32 {
        let mut cooldown = self.cooldown.write();
        let result = *cooldown;
        *cooldown = new_cooldown;

        result
    }

    #[inline]
    pub fn reset(&self, user_id: u32, mode: GameMode) {
        let now = Utc::now();
        *self.last_date.write() = now;
        let entry = TrackingEntry { user_id, mode };
        self.queue.write().push_decrease(entry, Reverse(now));
    }

    pub async fn update_last_date(
        &self,
        user_id: u32,
        mode: GameMode,
        new_date: DateTime<Utc>,
        psql: &Database,
    ) -> BotResult<()> {
        let entry = TrackingEntry { user_id, mode };

        if let Some(mut user) = self.users.get_mut(&entry) {
            if new_date > user.last_top_score {
                user.last_top_score = new_date;
                psql.update_osu_tracking_date(&entry, new_date).await?;
            }
        }

        Ok(())
    }

    #[inline]
    pub fn get_tracked(&self, user_id: u32, mode: GameMode) -> Option<(DateTime<Utc>, Channels)> {
        let entry = TrackingEntry { user_id, mode };

        self.users
            .get(&entry)
            .map(|user| (user.last_top_score, user.channels.to_owned()))
    }

    pub async fn pop(&self, entries: &mut Vec<(TrackingEntry, usize)>) {
        let len = self.queue.read().len();

        if len == 0 || self.stop_tracking.load(Ordering::Acquire) {
            time::sleep(time::Duration::from_secs(5)).await;

            return;
        }

        let last_date = *self.last_date.read();

        // Calculate how many users need to be popped for this iteration
        // so that _all_ users will be popped within the next INTERVAL
        let interval = last_date + *self.interval.read() - Utc::now();
        let ms_per_track = interval.num_milliseconds() as f32 / len as f32;
        let amount = (*self.cooldown.read() / ms_per_track).max(1.0);
        let delay = (ms_per_track * amount) as u64;
        time::sleep(time::Duration::from_millis(delay)).await;

        // Pop users and return them
        let mut queue = self.queue.write();

        for _ in 0..amount as usize {
            if let Some(entry) = queue.pop().map(|(entry, _)| entry) {
                let guard = self.users.get(&entry);

                if let Some(amount) = guard.and_then(|g| g.value().channels.values().max().copied())
                {
                    entries.push((entry, amount));
                }
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
            self.queue.write().remove(&entry);
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
                    // tracking_debug!("Removing ({},{}) from tracking", user_id, mode);
                    psql.remove_osu_tracking(user_id, mode).await?;
                    self.queue.write().remove(&key);
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
                self.queue.write().remove(&key);
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
        last_top_score: DateTime<Utc>,
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
                let now = Utc::now();
                *self.last_date.write() = now;
                let entry = TrackingEntry { user_id, mode };
                self.queue.write().push(entry, Reverse(now));
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
