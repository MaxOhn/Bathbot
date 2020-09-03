mod tracking_loop;

pub use tracking_loop::{process_tracking, tracking_loop};

use crate::{database::TrackingUser, BotResult, Database};

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use priority_queue::PriorityQueue;
use rosu::models::GameMode;
use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    iter,
};
use tokio::{sync::RwLock, time};
use twilight::model::id::ChannelId;

lazy_static::lazy_static! {
    static ref INTERVAL: Duration = Duration::seconds(600);
    static ref COOLDOWN: Duration = Duration::seconds(5);
}

type TrackingQueue = RwLock<PriorityQueue<(u32, GameMode), Reverse<DateTime<Utc>>>>;

pub struct OsuTracking {
    queue: TrackingQueue,
    users: DashMap<(u32, GameMode), TrackingUser>,
    last_date: Option<DateTime<Utc>>,
}

impl OsuTracking {
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
            last_date: None,
        })
    }

    pub async fn reset(&mut self, user: u32, mode: GameMode) {
        let mut queue = self.queue.write().await;
        let now = Utc::now();
        self.last_date = Some(now);
        queue.push_decrease((user, mode), Reverse(now));
    }

    pub async fn update_last_date(
        &mut self,
        user_id: u32,
        mode: GameMode,
        new_date: DateTime<Utc>,
        psql: &Database,
    ) -> BotResult<()> {
        if let Some(mut tracked_user) = self.users.get_mut(&(user_id, mode)) {
            tracked_user.last_top_score = new_date;
            psql.update_osu_tracking(user_id, mode, new_date, &tracked_user.channels)
                .await?;
        }
        Ok(())
    }

    pub fn get_tracked(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Option<(DateTime<Utc>, HashSet<ChannelId>)> {
        self.users
            .get(&(user_id, mode))
            .map(|user| (user.last_top_score, user.channels.to_owned()))
    }

    pub async fn pop(&mut self) -> Option<HashMap<(u32, GameMode), DateTime<Utc>>> {
        let len = {
            let queue = self.queue.read().await;
            queue.len()
        };
        debug!(
            "Popping... [len: {} ~ last_date: {:?}]",
            len, self.last_date
        );
        // Wait a duration of at least COOLDOWN
        let delay = match self.last_date.map(|date| Utc::now() - date) {
            Some(duration) => COOLDOWN.max(duration).num_milliseconds() as u64,
            None => 0,
        };
        time::delay_for(time::Duration::from_millis(delay)).await;
        // Calculate how many users need to be popped for this iteration
        // so that _all_ users will be popped within the next INTERVAL
        let interval = self.last_date? + *INTERVAL - Utc::now();
        let ms_per_track = interval.num_milliseconds() as f32 / len as f32;
        let amount = (COOLDOWN.num_milliseconds() as f32 / ms_per_track).max(1.0) as usize;
        debug!("Waited {}ms ~ ms_per_track: {}", delay, ms_per_track);
        // Pop users and return them
        let elems = {
            let mut queue = self.queue.write().await;
            iter::repeat_with(|| queue.pop().map(|(key, _)| key))
                .take(amount)
                .flatten()
                .map(|key| {
                    let last_top_score = self.users.get(&key).unwrap().last_top_score;
                    ((key.0, key.1), last_top_score)
                })
                .collect()
        };
        self.last_date = Some(Utc::now());
        Some(elems)
    }

    pub async fn remove(
        &mut self,
        user_id: u32,
        mode: GameMode,
        channel: ChannelId,
        psql: &Database,
    ) -> BotResult<bool> {
        let key = (user_id, mode);
        let removed = self
            .users
            .get_mut(&key)
            .map(|mut guard| guard.value_mut().remove_channel(channel));
        if let None | Some(false) = removed {
            return Ok(false);
        }
        let guard = self.users.get(&key).unwrap();
        let tracked_user = guard.value();
        if tracked_user.channels.is_empty() {
            psql.remove_osu_tracking(user_id, mode).await?;
            self.queue.write().await.remove(&key);
            self.users.remove(&key);
        } else {
            psql.update_osu_tracking(
                user_id,
                mode,
                tracked_user.last_top_score,
                &tracked_user.channels,
            )
            .await?
        }
        Ok(true)
    }

    pub async fn remove_all(
        &mut self,
        channel: ChannelId,
        mode: Option<GameMode>,
        psql: &Database,
    ) -> BotResult<usize> {
        let iter = self.users.iter_mut().filter(|guard| match mode {
            Some(mode) => guard.key().1 == mode,
            None => true,
        });
        let mut count = 0;
        for mut guard in iter {
            if !guard.value_mut().remove_channel(channel) {
                continue;
            }
            let key = guard.key();
            let tracked_user = guard.value();
            if tracked_user.channels.is_empty() {
                psql.remove_osu_tracking(key.0, key.1).await?;
                self.queue.write().await.remove(&key);
                self.users.remove(&key);
            } else {
                psql.update_osu_tracking(
                    key.0,
                    key.1,
                    tracked_user.last_top_score,
                    &tracked_user.channels,
                )
                .await?
            }
            count += 1;
        }
        Ok(count)
    }

    pub async fn add(
        &mut self,
        user_id: u32,
        mode: GameMode,
        channel: ChannelId,
        last_top_score: DateTime<Utc>,
        psql: &Database,
    ) -> BotResult<bool> {
        let key = (user_id, mode);
        match self.users.get_mut(&key) {
            Some(mut guard) => {
                if guard.value().channels.contains(&channel) {
                    return Ok(false);
                } else {
                    let value = guard.value_mut();
                    value.channels.insert(channel);
                    psql.update_osu_tracking(user_id, mode, value.last_top_score, &value.channels)
                        .await?;
                }
            }
            None => {
                psql.insert_osu_tracking(user_id, mode, last_top_score, channel.0)
                    .await?;
                let tracking_user = TrackingUser::new(user_id, mode, last_top_score, channel);
                self.users.insert(key, tracking_user);
                let now = Utc::now();
                self.last_date.replace(now);
                let mut queue = self.queue.write().await;
                queue.push((user_id, mode), Reverse(now));
            }
        }
        Ok(true)
    }

    pub fn list(&self, channel: ChannelId) -> Vec<(u32, GameMode)> {
        self.users
            .iter()
            .filter(|guard| guard.value().channels.contains(&channel))
            .map(|guard| *guard.key())
            .collect()
    }
}
