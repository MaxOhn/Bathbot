use super::is_default;
use crate::core::cache::{Cache, CachedUser};

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use twilight::model::{
    channel::{
        permission_overwrite::PermissionOverwrite, ChannelType, GuildChannel, PrivateChannel,
    },
    id::{ChannelId, GuildId},
};

const NO_PERMISSIONS: &[PermissionOverwrite] = &[];

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CachedChannel {
    TextChannel {
        #[serde(rename = "a")]
        id: ChannelId,
        #[serde(rename = "b")]
        guild_id: GuildId,
        #[serde(rename = "c", default, skip_serializing_if = "is_default")]
        position: i64,
        // should be always present in guild create,
        #[serde(rename = "d", default, skip_serializing_if = "is_default")]
        permission_overrides: Vec<PermissionOverwrite>,
        #[serde(rename = "e")]
        name: String,
        #[serde(rename = "f", default, skip_serializing_if = "is_default")]
        topic: Option<String>,
        #[serde(rename = "g", default, skip_serializing_if = "is_default")]
        nsfw: bool,
        #[serde(rename = "h", default, skip_serializing_if = "is_default")]
        slowmode: Option<u64>,
        #[serde(rename = "i", default, skip_serializing_if = "is_default")]
        parent_id: Option<ChannelId>,
    },
    DM {
        id: ChannelId,
        receiver: Arc<CachedUser>,
    },
    VoiceChannel {
        #[serde(rename = "a")]
        id: ChannelId,
        #[serde(rename = "b")]
        guild_id: GuildId,
        #[serde(rename = "c", default, skip_serializing_if = "is_default")]
        position: i64,
        // should be always present in guild create,
        #[serde(rename = "d", default, skip_serializing_if = "is_default")]
        permission_overrides: Vec<PermissionOverwrite>,
        #[serde(rename = "e")]
        name: String,
        #[serde(rename = "f", default, skip_serializing_if = "is_default")]
        bitrate: u64,
        #[serde(rename = "g", default, skip_serializing_if = "is_default")]
        user_limit: Option<u64>,
        #[serde(rename = "h", default, skip_serializing_if = "is_default")]
        parent_id: Option<ChannelId>,
    },
    GroupDM {
        id: ChannelId,
        receivers: Vec<Arc<CachedUser>>,
    },
    Category {
        #[serde(rename = "a")]
        id: ChannelId,
        #[serde(rename = "b")]
        guild_id: GuildId,
        #[serde(rename = "c", default, skip_serializing_if = "is_default")]
        position: i64,
        // should be always present in guild create,
        #[serde(rename = "d", default, skip_serializing_if = "is_default")]
        permission_overrides: Vec<PermissionOverwrite>,
        #[serde(rename = "e")]
        name: String,
    },
    AnnouncementsChannel {
        #[serde(rename = "a")]
        id: ChannelId,
        #[serde(rename = "b")]
        guild_id: GuildId,
        #[serde(rename = "c", default, skip_serializing_if = "is_default")]
        position: i64,
        // should be always present in guild create,
        #[serde(rename = "d", default, skip_serializing_if = "is_default")]
        permission_overrides: Vec<PermissionOverwrite>,
        #[serde(rename = "e")]
        name: String,
        #[serde(rename = "f", default, skip_serializing_if = "is_default")]
        parent_id: Option<ChannelId>,
    },
    StoreChannel {
        id: ChannelId,
        guild_id: GuildId,
        position: i64,
        // should be always present in guild create,
        name: String,
        parent_id: Option<ChannelId>,
        permission_overrides: Vec<PermissionOverwrite>, // they might not allow for text, but they do have overrides
    },
}

impl CachedChannel {
    /// returns the channel ID
    /// Note this is different from userid when DMing users
    pub fn get_id(&self) -> ChannelId {
        match self {
            CachedChannel::TextChannel { id, .. } => *id,
            CachedChannel::DM { id, .. } => *id,
            CachedChannel::VoiceChannel { id, .. } => *id,
            CachedChannel::GroupDM { id, .. } => *id,
            CachedChannel::Category { id, .. } => *id,
            CachedChannel::AnnouncementsChannel { id, .. } => *id,
            CachedChannel::StoreChannel { id, .. } => *id,
        }
    }

    /// Returns the guild id
    pub fn get_guild_id(&self) -> Option<GuildId> {
        match self {
            CachedChannel::TextChannel { guild_id, .. } => Some(*guild_id),
            CachedChannel::DM { .. } => None,
            CachedChannel::VoiceChannel { guild_id, .. } => Some(*guild_id),
            CachedChannel::GroupDM { .. } => None,
            CachedChannel::Category { guild_id, .. } => Some(*guild_id),
            CachedChannel::AnnouncementsChannel { guild_id, .. } => Some(*guild_id),
            CachedChannel::StoreChannel { guild_id, .. } => Some(*guild_id),
        }
    }

    /// Gets the position of this channel
    /// returns 0 for DM (group) channels
    pub fn get_position(&self) -> i64 {
        match self {
            CachedChannel::TextChannel { position, .. } => *position,
            CachedChannel::DM { .. } => 0,
            CachedChannel::VoiceChannel { position, .. } => *position,
            CachedChannel::GroupDM { .. } => 0,
            CachedChannel::Category { position, .. } => *position,
            CachedChannel::AnnouncementsChannel { position, .. } => *position,
            CachedChannel::StoreChannel { position, .. } => *position,
        }
    }

    pub fn get_name(&self) -> &str {
        match self {
            CachedChannel::TextChannel { name, .. } => name,
            CachedChannel::DM { .. } => "",
            CachedChannel::VoiceChannel { name, .. } => name,
            CachedChannel::GroupDM { .. } => "",
            CachedChannel::Category { name, .. } => name,
            CachedChannel::AnnouncementsChannel { name, .. } => name,
            CachedChannel::StoreChannel { name, .. } => name,
        }
    }

    pub fn get_topic(&self) -> &Option<String> {
        match self {
            CachedChannel::TextChannel { topic, .. } => topic,
            CachedChannel::DM { .. } => &None,
            CachedChannel::VoiceChannel { .. } => &None,
            CachedChannel::GroupDM { .. } => &None,
            CachedChannel::Category { .. } => &None,
            CachedChannel::AnnouncementsChannel { .. } => &None,
            CachedChannel::StoreChannel { .. } => &None,
        }
    }

    pub fn get_permission_overrides(&self) -> &[PermissionOverwrite] {
        match self {
            CachedChannel::TextChannel {
                permission_overrides,
                ..
            } => permission_overrides,
            CachedChannel::DM { .. } => NO_PERMISSIONS,
            CachedChannel::VoiceChannel {
                permission_overrides,
                ..
            } => permission_overrides,
            CachedChannel::GroupDM { .. } => NO_PERMISSIONS,
            CachedChannel::Category {
                permission_overrides,
                ..
            } => permission_overrides,
            CachedChannel::AnnouncementsChannel {
                permission_overrides,
                ..
            } => permission_overrides,
            CachedChannel::StoreChannel {
                permission_overrides,
                ..
            } => permission_overrides,
        }
    }

    pub fn is_nsfw(&self) -> bool {
        match self {
            CachedChannel::TextChannel { nsfw, .. } => *nsfw,
            _ => false,
        }
    }

    pub fn is_dm(&self) -> bool {
        match self {
            CachedChannel::DM { .. } => true,
            _ => false,
        }
    }
}

impl CachedChannel {
    pub fn from_guild_channel(channel: &GuildChannel, guild_id: GuildId) -> Self {
        let (
            kind,
            id,
            position,
            permission_overrides,
            name,
            topic,
            nsfw,
            slowmode,
            parent_id,
            bitrate,
            user_limit,
        ) = match channel {
            GuildChannel::Category(category) => (
                category.kind,
                category.id,
                category.position,
                category.permission_overwrites.clone(),
                category.name.clone(),
                None,
                false,
                None,
                None,
                0,
                None,
            ),
            GuildChannel::Text(text) => (
                text.kind,
                text.id,
                text.position,
                text.permission_overwrites.clone(),
                text.name.clone(),
                text.topic.clone(),
                text.nsfw,
                text.rate_limit_per_user,
                text.parent_id,
                0,
                None,
            ),
            GuildChannel::Voice(voice) => (
                voice.kind,
                voice.id,
                voice.position,
                voice.permission_overwrites.clone(),
                voice.name.clone(),
                None,
                false,
                None,
                voice.parent_id,
                voice.bitrate,
                voice.user_limit,
            ),
        };

        match kind {
            ChannelType::GuildText => CachedChannel::TextChannel {
                id,
                guild_id,
                position,
                permission_overrides,
                name,
                topic,
                nsfw,
                slowmode,
                parent_id,
            },
            ChannelType::Private => unreachable!(),
            ChannelType::GuildVoice => CachedChannel::VoiceChannel {
                id,
                guild_id,
                position,
                permission_overrides,
                name,
                bitrate,
                user_limit,
                parent_id,
            },
            ChannelType::Group => unreachable!(),
            ChannelType::GuildCategory => CachedChannel::Category {
                id,
                guild_id,
                position,
                permission_overrides,
                name,
            },
            ChannelType::GuildNews => CachedChannel::AnnouncementsChannel {
                id,
                guild_id,
                position,
                permission_overrides,
                name,
                parent_id,
            },
            ChannelType::GuildStore => CachedChannel::StoreChannel {
                id,
                guild_id,
                position,
                name,
                parent_id,
                permission_overrides,
            },
        }
    }

    pub fn from_private(channel: &PrivateChannel, cache: &Cache) -> Self {
        if channel.recipients.len() == 1 {
            CachedChannel::DM {
                id: channel.id,
                receiver: cache.get_or_insert_user(&channel.recipients[0]),
            }
        } else {
            CachedChannel::GroupDM {
                id: channel.id,
                receivers: channel
                    .recipients
                    .iter()
                    .map(|user| cache.get_or_insert_user(user))
                    .collect(),
            }
        }
    }
}
