use crate::{
    commands::fun::BackGroundGame,
    database::{MySQL, StreamTrack},
    scraper::Scraper,
    streams::Twitch,
    util::globals::AUTHORITY_ROLES,
};

use chrono::{DateTime, Utc};
use rosu::backend::Osu as OsuClient;
use serenity::{
    framework::standard::{Args, Delimiter},
    model::id::{ChannelId, GuildId, MessageId, RoleId, UserId},
    prelude::*,
};
use sqlx::{mysql::MySqlRow, FromRow, Row};
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

pub struct Guild {
    pub guild_id: GuildId,
    pub with_lyrics: bool,
    pub authorities: Vec<String>,
}

impl Guild {
    pub fn new(guild_id: u64) -> Self {
        let mut authorities = Vec::new();
        let mut args = Args::new(AUTHORITY_ROLES, &[Delimiter::Single(' ')]);
        while !args.is_empty() {
            authorities.push(args.single_quoted().unwrap());
        }
        Self {
            guild_id: GuildId(guild_id),
            with_lyrics: true,
            authorities,
        }
    }
}

impl<'c> FromRow<'c, MySqlRow> for Guild {
    fn from_row(row: &MySqlRow) -> Result<Guild, sqlx::Error> {
        let role_string: &str = row.get("authorities");
        let mut authorities = Vec::new();
        let mut args = Args::new(role_string, &[Delimiter::Single(' ')]);
        while !args.is_empty() {
            authorities.push(args.single_quoted().unwrap());
        }
        Ok(Guild {
            guild_id: GuildId(row.get("guild_id")),
            with_lyrics: row.get("with_lyrics"),
            authorities,
        })
    }
}

pub struct Guilds;
impl TypeMapKey for Guilds {
    type Value = HashMap<GuildId, Guild>;
}

pub struct BgGames;
impl TypeMapKey for BgGames {
    type Value = HashMap<ChannelId, BackGroundGame>;
}

pub struct BgVerified;
impl TypeMapKey for BgVerified {
    type Value = HashSet<UserId>;
}
