mod tracking_loop;

pub use tracking_loop::{process_tracking, tracking_loop};

use crate::{database::TrackingUser, BotResult, Database};

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use hashbrown::HashMap;
use priority_queue::PriorityQueue;
use rosu_v2::model::GameMode;
use std::{
    cmp::Reverse,
    iter,
    sync::atomic::{AtomicBool, Ordering},
};
use tokio::{sync::RwLock, time};
use twilight_model::id::ChannelId;

lazy_static::lazy_static! {
    pub static ref OSU_TRACKING_INTERVAL: Duration = Duration::minutes(120);
    pub static ref OSU_TRACKING_COOLDOWN: f32 = 5000.0; // ms
}

type TrackingQueue = RwLock<PriorityQueue<(u32, GameMode), Reverse<DateTime<Utc>>>>;

pub struct TrackingStats {
    pub next_pop: (u32, GameMode),
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

pub struct OsuTracking {
    queue: TrackingQueue,
    users: DashMap<(u32, GameMode), TrackingUser>,
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
            .map(|guard| {
                let value = guard.value();
                ((value.user_id, value.mode), Reverse(Utc::now()))
            })
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

    pub async fn stats(&self) -> TrackingStats {
        let next_pop = self.queue.read().await.peek().map(|(&key, _)| key).unwrap();
        let users = self.users.len();
        let queue = self.queue.read().await.len();
        let last_pop = *self.last_date.read().await;
        let interval = *self.interval.read().await;
        let cooldown = *self.cooldown.read().await;
        let tracking = !self.stop_tracking.load(Ordering::Relaxed);

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
    pub async fn set_cooldown(&self, new_cooldown: f32) -> f32 {
        let mut cooldown = self.cooldown.write().await;
        let result = *cooldown;
        *cooldown = new_cooldown;

        result
    }

    #[inline]
    pub async fn reset(&self, user: u32, mode: GameMode) {
        let now = Utc::now();
        *self.last_date.write().await = now;
        self.queue
            .write()
            .await
            .push_decrease((user, mode), Reverse(now));
    }

    pub async fn update_last_date(
        &self,
        user_id: u32,
        mode: GameMode,
        new_date: DateTime<Utc>,
        psql: &Database,
    ) -> BotResult<()> {
        if let Some(mut tracked_user) = self.users.get_mut(&(user_id, mode)) {
            if new_date > tracked_user.last_top_score {
                tracked_user.last_top_score = new_date;
                psql.update_osu_tracking(user_id, mode, new_date, &tracked_user.channels)
                    .await?;
            } else {
                debug!(
                    "[update_last_date] ({},{})'s date {} is already greater than {}",
                    user_id, mode, tracked_user.last_top_score, new_date
                );
            }
        } else {
            debug!(
                "[update_last_date] ({},{}) not found in users",
                user_id, mode
            );
        }

        Ok(())
    }

    #[inline]
    pub fn get_tracked(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Option<(DateTime<Utc>, HashMap<ChannelId, usize>)> {
        self.users
            .get(&(user_id, mode))
            .map(|user| (user.last_top_score, user.channels.to_owned()))
    }

    pub async fn pop(&self) -> Option<Vec<(u32, GameMode)>> {
        let len = self.queue.read().await.len();

        if len == 0 || self.stop_tracking.load(Ordering::Relaxed) {
            return None;
        }

        let last_date = *self.last_date.read().await;

        // Calculate how many users need to be popped for this iteration
        // so that _all_ users will be popped within the next INTERVAL
        let interval = last_date + *self.interval.read().await - Utc::now();
        let ms_per_track = interval.num_milliseconds() as f32 / len as f32;
        let amount = (*self.cooldown.read().await / ms_per_track).max(1.0);
        let delay = (ms_per_track * amount) as u64;

        debug!(
            "[Popping] All: {} ~ Last date: {:?} ~ Amount: {} ~ Delay: {}ms",
            len, last_date, amount, delay
        );

        time::sleep(time::Duration::from_millis(delay)).await;

        // Pop users and return them
        let elems = {
            let mut queue = self.queue.write().await;

            iter::repeat_with(|| queue.pop().map(|(key, _)| key))
                .take(amount as usize)
                .flatten()
                .collect()
        };

        Some(elems)
    }

    pub async fn remove_user(
        &self,
        user_id: u32,
        channel: ChannelId,
        psql: &Database,
    ) -> BotResult<()> {
        let removed: Vec<_> = self
            .users
            .iter_mut()
            .filter(|guard| guard.key().0 == user_id)
            .filter_map(
                |mut guard| match guard.value_mut().remove_channel(channel) {
                    true => Some(guard.key().1),
                    false => None,
                },
            )
            .collect();

        for mode in removed {
            let key = (user_id, mode);

            match self
                .users
                .get(&key)
                .map(|guard| guard.value().channels.is_empty())
            {
                Some(true) => {
                    debug!("Removing ({},{}) from tracking", user_id, mode);
                    psql.remove_osu_tracking(user_id, mode).await?;
                    self.queue.write().await.remove(&key);
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
        channel: ChannelId,
        mode: Option<GameMode>,
        psql: &Database,
    ) -> BotResult<usize> {
        let mut count = 0;

        let to_remove: Vec<_> = self
            .users
            .iter_mut()
            .filter(|guard| match mode {
                Some(mode) => guard.key().1 == mode,
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
                debug!("Removing {:?} from tracking (all)", key);
                psql.remove_osu_tracking(key.0, key.1).await?;
                self.queue.write().await.remove(&key);
                self.users.remove(&key);
            } else {
                let guard = match self.users.get(&key) {
                    Some(guard) => guard,
                    None => continue,
                };

                let user = guard.value();
                let (user_id, mode) = key;

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
        channel: ChannelId,
        limit: usize,
        psql: &Database,
    ) -> BotResult<bool> {
        let key = (user_id, mode);

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
                debug!("Inserting {:?} for tracking", key);

                psql.insert_osu_tracking(user_id, mode, last_top_score, channel, limit)
                    .await?;

                let tracking_user =
                    TrackingUser::new(user_id, mode, last_top_score, channel, limit);

                self.users.insert(key, tracking_user);
                let now = Utc::now();
                *self.last_date.write().await = now;
                self.queue.write().await.push((user_id, mode), Reverse(now));
            }
        }

        Ok(true)
    }

    pub fn list(&self, channel: ChannelId) -> Vec<(u32, GameMode, usize)> {
        self.users
            .iter()
            .filter_map(|guard| {
                let limit = *guard.value().channels.get(&channel)?;
                let (user_id, mode) = guard.key();

                Some((*user_id, *mode, limit))
            })
            .collect()
    }
}
