use crate::{
    commands::fun::BackGroundGame,
    database::{Guild as GuildDB, MySQL, StreamTrack},
    scraper::Scraper,
    streams::Twitch,
};

use chrono::{DateTime, Utc};
use hey_listen::{sync::ParallelDispatcher as Dispatcher, RwLock as HlRwLock};
use rosu::backend::Osu as OsuClient;
use serenity::{
    model::id::{ChannelId, GuildId, MessageId, RoleId, UserId},
    prelude::*,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::{Hash, Hasher},
    sync::Arc,
};
use white_rabbit::Scheduler;

pub struct CommandCounter;
impl TypeMapKey for CommandCounter {
    type Value = HashMap<String, u32>;
}

pub struct Osu;
impl TypeMapKey for Osu {
    type Value = OsuClient;
}

impl TypeMapKey for Scraper {
    type Value = Scraper;
}

impl TypeMapKey for MySQL {
    type Value = MySQL;
}

pub struct DiscordLinks;
impl TypeMapKey for DiscordLinks {
    type Value = HashMap<u64, String>;
}

pub struct BootTime;
impl TypeMapKey for BootTime {
    type Value = DateTime<Utc>;
}

pub struct PerformanceCalculatorLock;
impl TypeMapKey for PerformanceCalculatorLock {
    type Value = Arc<Mutex<()>>;
}

pub struct SchedulerKey;
impl TypeMapKey for SchedulerKey {
    type Value = Arc<RwLock<Scheduler>>;
}

pub struct ReactionTracker;
impl TypeMapKey for ReactionTracker {
    type Value = HashMap<(ChannelId, MessageId), RoleId>;
}

pub struct TwitchUsers;
impl TypeMapKey for TwitchUsers {
    type Value = HashMap<String, u64>;
}

pub struct StreamTracks;
impl TypeMapKey for StreamTracks {
    type Value = HashSet<StreamTrack>;
}

pub struct OnlineTwitch;
impl TypeMapKey for OnlineTwitch {
    type Value = HashSet<u64>;
}

impl TypeMapKey for Twitch {
    type Value = Twitch;
}

pub struct ResponseOwner;
impl TypeMapKey for ResponseOwner {
    type Value = (VecDeque<MessageId>, HashMap<MessageId, UserId>);
}

pub struct Guilds;
impl TypeMapKey for Guilds {
    type Value = HashMap<GuildId, GuildDB>;
}

#[derive(Debug, Clone)]
pub enum DispatchEvent {
    BgMsgEvent {
        channel: ChannelId,
        user: UserId,
        content: String,
    },
}

impl PartialEq for DispatchEvent {
    fn eq(&self, other: &DispatchEvent) -> bool {
        match (self, other) {
            (
                DispatchEvent::BgMsgEvent {
                    channel: self_channel,
                    ..
                },
                DispatchEvent::BgMsgEvent {
                    channel: other_channel,
                    ..
                },
            ) => self_channel == other_channel,
        }
    }
}

impl Eq for DispatchEvent {}

impl Hash for DispatchEvent {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

pub struct DispatcherKey;
impl TypeMapKey for DispatcherKey {
    type Value = Arc<RwLock<Dispatcher<DispatchEvent>>>;
}

pub struct BgGameKey;
impl TypeMapKey for BgGameKey {
    type Value = HashMap<ChannelId, Arc<HlRwLock<BackGroundGame>>>;
}
