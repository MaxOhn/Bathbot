use crate::{
    commands::fun::BackGroundGame,
    database::{Guild as GuildDB, MySQL, StreamTrack},
    scraper::Scraper,
    streams::Twitch,
};

use chrono::{DateTime, Utc};
use rosu::backend::Osu as OsuClient;
use serenity::{
    model::id::{ChannelId, GuildId, MessageId, RoleId},
    prelude::*,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

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

pub struct Guilds;
impl TypeMapKey for Guilds {
    type Value = HashMap<GuildId, GuildDB>;
}

pub struct BgGames;
impl TypeMapKey for BgGames {
    type Value = HashMap<ChannelId, BackGroundGame>;
}
