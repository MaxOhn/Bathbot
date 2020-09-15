use super::is_default;
use crate::core::cache::{Cache, CachedChannel, CachedEmoji, CachedMember, CachedRole};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use twilight_model::{
    guild::{Guild, PartialGuild},
    id::{ChannelId, GuildId, RoleId, UserId},
};

#[derive(Debug)]
pub struct CachedGuild {
    // api fields
    pub id: GuildId,
    pub name: String,
    pub icon: Option<String>,
    pub owner_id: UserId,
    pub roles: DashMap<RoleId, Arc<CachedRole>>,
    pub emoji: Vec<Arc<CachedEmoji>>,
    pub features: Vec<String>,
    pub unavailable: bool,
    pub members: DashMap<UserId, Arc<CachedMember>>,
    pub channels: DashMap<ChannelId, Arc<CachedChannel>>,
    // use our own version, easier to work with than twilight's enum
    pub max_presences: Option<u64>,
    // defaults to 25_000 if null in the guild create
    pub max_members: Option<u64>,
    // should always be present in guild create, but option just in case
    pub description: Option<String>,
    pub preferred_locale: String,

    // own fields
    pub complete: AtomicBool,
    pub member_count: AtomicU64,
}

impl From<Guild> for CachedGuild {
    fn from(guild: Guild) -> Self {
        let mut cached_guild = CachedGuild {
            id: guild.id,
            name: guild.name,
            icon: guild.icon,
            owner_id: guild.owner_id,
            roles: DashMap::new(),
            emoji: vec![],
            features: guild.features,
            unavailable: false,
            members: DashMap::new(),
            channels: DashMap::new(),
            max_presences: guild.max_presences,
            max_members: guild.max_members,
            description: guild.description,
            preferred_locale: guild.preferred_locale,
            complete: AtomicBool::new(false),
            member_count: AtomicU64::new(0),
        };
        // handle roles
        for (role_id, role) in guild.roles {
            cached_guild
                .roles
                .insert(role_id, Arc::new(CachedRole::from_role(&role)));
        }
        // channels
        for (channel_id, channel) in guild.channels {
            cached_guild.channels.insert(
                channel_id,
                Arc::new(CachedChannel::from_guild_channel(&channel, guild.id)),
            );
        }
        // emoji
        for (_, emoji) in guild.emojis {
            cached_guild.emoji.push(Arc::new(CachedEmoji::from(emoji)));
        }
        cached_guild
    }
}

impl CachedGuild {
    pub fn defrost(cache: &Cache, cold_guild: ColdStorageGuild) -> Self {
        let mut guild = CachedGuild {
            id: cold_guild.id,
            name: cold_guild.name,
            icon: cold_guild.icon,
            owner_id: cold_guild.owner_id,
            roles: DashMap::new(),
            emoji: vec![],
            features: vec![],
            unavailable: false,
            members: DashMap::new(),
            channels: DashMap::new(),
            max_presences: cold_guild.max_presences,
            max_members: cold_guild.max_members,
            description: cold_guild.description,
            preferred_locale: cold_guild.preferred_locale,
            complete: AtomicBool::new(true),
            member_count: AtomicU64::new(cold_guild.members.len() as u64),
        };
        for role in cold_guild.roles {
            guild.roles.insert(role.id, Arc::new(role));
        }
        for member in cold_guild.members {
            if let Some(user) = cache.get_user(member.user_id) {
                user.mutual_servers.fetch_add(1, Ordering::SeqCst);
            }
            guild.members.insert(member.user_id, Arc::new(member));
        }
        for channel in cold_guild.channels {
            guild.channels.insert(channel.get_id(), Arc::new(channel));
        }
        for emoji in cold_guild.emoji {
            guild.emoji.push(Arc::new(emoji));
        }
        guild
    }

    pub fn update(&self, other: &PartialGuild) -> Self {
        let guild = CachedGuild {
            id: other.id,
            name: other.name.clone(),
            icon: other.icon.clone(),
            owner_id: other.owner_id,
            roles: DashMap::new(),
            emoji: self.emoji.clone(),
            features: other.features.clone(),
            unavailable: false,
            members: self.members.clone(),
            channels: self.channels.clone(),
            max_presences: other.max_presences,
            max_members: other.max_members,
            description: other.description.clone(),
            preferred_locale: other.preferred_locale.clone(),
            complete: AtomicBool::new(self.complete.load(Ordering::SeqCst)),
            member_count: AtomicU64::new(self.member_count.load(Ordering::SeqCst)),
        };
        for role in other.roles.values() {
            guild
                .roles
                .insert(role.id, Arc::new(CachedRole::from_role(role)));
        }
        guild
    }

    pub fn get_role(&self, role_id: RoleId) -> Option<Arc<CachedRole>> {
        self.roles.get(&role_id).map(|guard| guard.value().clone())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ColdStorageGuild {
    #[serde(rename = "a")]
    pub id: GuildId,
    #[serde(rename = "b")]
    pub name: String,
    #[serde(rename = "c", default, skip_serializing_if = "is_default")]
    pub icon: Option<String>,
    #[serde(rename = "f")]
    pub owner_id: UserId,
    #[serde(rename = "l")]
    pub roles: Vec<CachedRole>,
    #[serde(rename = "m")]
    pub emoji: Vec<CachedEmoji>,
    #[serde(rename = "n", default, skip_serializing_if = "is_default")]
    pub features: Vec<String>,
    #[serde(rename = "o")]
    pub members: Vec<CachedMember>,
    #[serde(rename = "p")]
    pub channels: Vec<CachedChannel>,
    #[serde(rename = "q", default, skip_serializing_if = "is_default")]
    pub max_presences: Option<u64>,
    #[serde(rename = "r", default, skip_serializing_if = "is_default")]
    pub max_members: Option<u64>,
    #[serde(rename = "s", default, skip_serializing_if = "is_default")]
    pub description: Option<String>,
    #[serde(rename = "w", default, skip_serializing_if = "is_default")]
    pub preferred_locale: String,
}

impl From<Arc<CachedGuild>> for ColdStorageGuild {
    fn from(guild: Arc<CachedGuild>) -> Self {
        let roles = guild
            .roles
            .iter()
            .map(|guard| CachedRole::from(guard.value().clone()))
            .collect();
        guild.roles.clear();

        let emoji = guild
            .emoji
            .iter()
            .map(|emoji| emoji.as_ref().clone())
            .collect();

        let members = guild
            .members
            .iter()
            .map(|member| member.duplicate())
            .collect();
        guild.members.clear();

        let channels = guild
            .channels
            .iter()
            .map(|channel| match channel.as_ref() {
                CachedChannel::TextChannel {
                    id,
                    guild_id,
                    permission_overrides,
                    name,
                    parent_id,
                } => CachedChannel::TextChannel {
                    id: *id,
                    guild_id: *guild_id,
                    permission_overrides: permission_overrides.clone(),
                    name: name.clone(),
                    parent_id: *parent_id,
                },
                CachedChannel::DM { id, receiver } => CachedChannel::DM {
                    id: *id,
                    receiver: receiver.clone(),
                },
                CachedChannel::VoiceChannel {
                    id,
                    guild_id,
                    permission_overrides,
                    name,
                    parent_id,
                } => CachedChannel::VoiceChannel {
                    id: *id,
                    guild_id: *guild_id,
                    permission_overrides: permission_overrides.clone(),
                    name: name.clone(),
                    parent_id: *parent_id,
                },
                CachedChannel::GroupDM { id, receivers } => CachedChannel::GroupDM {
                    id: *id,
                    receivers: receivers.clone(),
                },
                CachedChannel::Category {
                    id,
                    guild_id,
                    permission_overrides,
                    name,
                } => CachedChannel::Category {
                    id: *id,
                    guild_id: *guild_id,
                    permission_overrides: permission_overrides.clone(),
                    name: name.clone(),
                },
                CachedChannel::AnnouncementsChannel {
                    id,
                    guild_id,
                    permission_overrides,
                    name,
                    parent_id,
                } => CachedChannel::AnnouncementsChannel {
                    id: *id,
                    guild_id: *guild_id,
                    permission_overrides: permission_overrides.clone(),
                    name: name.clone(),
                    parent_id: *parent_id,
                },
                CachedChannel::StoreChannel {
                    id,
                    guild_id,
                    name,
                    parent_id,
                    permission_overrides,
                } => CachedChannel::StoreChannel {
                    id: *id,
                    guild_id: *guild_id,
                    name: name.clone(),
                    parent_id: *parent_id,
                    permission_overrides: permission_overrides.clone(),
                },
            })
            .collect();

        ColdStorageGuild {
            id: guild.id,
            name: guild.name.clone(),
            icon: guild.icon.clone(),
            owner_id: guild.owner_id,
            roles,
            emoji,
            features: guild.features.clone(),
            members,
            channels,
            max_presences: guild.max_presences,
            max_members: guild.max_members,
            description: guild.description.clone(),
            preferred_locale: guild.preferred_locale.clone(),
        }
    }
}
