use std::{collections::HashMap, ops::Deref};

use deadpool_redis::Pool;
use twilight_cache_inmemory::{
    model::{CachedGuildChannel, CachedTextChannel},
    ColdResumeFlags, InMemoryCache, ResourceType,
};
use twilight_gateway::shard::ResumeSession;
use twilight_model::{
    channel::permission_overwrite::PermissionOverwriteType,
    guild::Permissions,
    id::{ChannelId, GuildId, UserId},
};

use crate::util::constants::OWNER_USER_ID;

pub struct Cache(InMemoryCache);

impl Cache {
    pub async fn new(redis: &Pool) -> (Self, Option<HashMap<u64, ResumeSession>>) {
        let resource_types = ResourceType::CHANNEL
            | ResourceType::GUILD
            | ResourceType::MEMBER
            | ResourceType::REACTION
            | ResourceType::ROLE
            | ResourceType::USER_CURRENT
            | ResourceType::USER;

        let resume_flags = ColdResumeFlags::all();

        let config = InMemoryCache::builder()
            .resource_types(resource_types)
            .resume_flags(resume_flags)
            .build()
            .config();

        let (cache, resume_map) = InMemoryCache::from_redis(redis, config).await;

        let resume_map = resume_map.map(|resume_map| {
            resume_map
                .into_iter()
                .map(|(key, (session_id, sequence))| {
                    let session = ResumeSession {
                        session_id,
                        sequence,
                    };

                    (key, session)
                })
                .collect()
        });

        (Self(cache), resume_map)
    }

    pub fn is_guild_owner(&self, guild_id: GuildId, user_id: UserId) -> bool {
        self.0
            .guild(guild_id)
            .map_or(false, |guild| guild.owner_id == user_id)
    }

    pub fn get_guild_permissions_for(
        &self,
        user_id: UserId,
        guild_id: Option<GuildId>,
    ) -> Permissions {
        if user_id.0 == OWNER_USER_ID {
            return Permissions::all();
        }

        let guild = match guild_id.and_then(|id| self.guild(id)) {
            Some(guild) => guild,
            None => return Permissions::empty(),
        };

        if guild.owner_id == user_id {
            return Permissions::all();
        }

        let member = match self.member(guild.id, user_id) {
            Some(member) => member,
            None => return Permissions::empty(),
        };

        let mut permissions = Permissions::empty();

        for &role_id in &member.roles {
            if let Some(role) = self.role(role_id) {
                if role.permissions.contains(Permissions::ADMINISTRATOR) {
                    return Permissions::all();
                }

                permissions |= role.permissions;
            }
        }

        permissions
    }

    pub fn get_channel_permissions_for(
        &self,
        user_id: UserId,
        channel_id: ChannelId,
        guild_id: Option<GuildId>,
    ) -> Permissions {
        let guild_id = if let Some(guild_id) = guild_id {
            guild_id
        } else {
            // Private channel
            return Permissions::SEND_MESSAGES
                | Permissions::EMBED_LINKS
                | Permissions::ATTACH_FILES
                | Permissions::USE_EXTERNAL_EMOJIS
                | Permissions::ADD_REACTIONS
                | Permissions::READ_MESSAGE_HISTORY;
        };

        let permissions = self.get_guild_permissions_for(user_id, Some(guild_id));

        if permissions.contains(Permissions::ADMINISTRATOR) {
            return Permissions::all();
        }

        let channel = match self.guild_channel(channel_id) {
            Some(CachedGuildChannel::Text(channel)) => Some(channel),
            Some(CachedGuildChannel::PrivateThread(thread)) => thread.parent_id.and_then(|id| {
                self.guild_channel(id).and_then(|channel| {
                    if let CachedGuildChannel::Text(channel) = channel {
                        Some(channel)
                    } else {
                        None
                    }
                })
            }),
            Some(CachedGuildChannel::PublicThread(thread)) => thread.parent_id.and_then(|id| {
                self.guild_channel(id).and_then(|channel| {
                    if let CachedGuildChannel::Text(channel) = channel {
                        Some(channel)
                    } else {
                        None
                    }
                })
            }),
            _ => None,
        };

        channel.map_or(permissions, |channel| {
            self._text_channel_permissions(permissions, user_id, guild_id, &channel)
        })
    }

    fn _text_channel_permissions(
        &self,
        mut permissions: Permissions,
        user_id: UserId,
        guild_id: GuildId,
        channel: &CachedTextChannel,
    ) -> Permissions {
        if let Some(member) = self.member(guild_id, user_id) {
            let mut everyone_allowed = Permissions::empty();
            let mut everyone_denied = Permissions::empty();
            let mut user_allowed = Permissions::empty();
            let mut user_denied = Permissions::empty();
            let mut role_allowed = Permissions::empty();
            let mut role_denied = Permissions::empty();

            for overwrite in &channel.permission_overwrites {
                match overwrite.kind {
                    PermissionOverwriteType::Member(member_id) => {
                        if member_id == user_id {
                            user_allowed |= overwrite.allow;
                            user_denied |= overwrite.deny;
                        }
                    }
                    PermissionOverwriteType::Role(role_id) => {
                        if role_id.0 == guild_id.0 {
                            everyone_allowed |= overwrite.allow;
                            everyone_denied |= overwrite.deny
                        } else if member.roles.contains(&role_id) {
                            role_allowed |= overwrite.allow;
                            role_denied |= overwrite.deny;
                        }
                    }
                }
            }

            permissions &= !everyone_denied;
            permissions |= everyone_allowed;

            permissions &= !role_denied;
            permissions |= role_allowed;

            permissions &= !user_denied;
            permissions |= user_allowed;
        }

        permissions
    }
}

impl Deref for Cache {
    type Target = InMemoryCache;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
