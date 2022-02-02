use std::ops::Deref;

use thiserror::Error;
use twilight_cache_inmemory::{
    model::{CachedGuild, CachedMember},
    GuildResource, InMemoryCache, InMemoryCacheStats, ResourceType,
};
use twilight_gateway::Event;
use twilight_model::{
    channel::{
        permission_overwrite::{PermissionOverwrite, PermissionOverwriteType},
        GuildChannel,
    },
    guild::{Permissions, Role},
    id::{
        marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
        Id,
    },
    user::CurrentUser,
};

use crate::util::constants::OWNER_USER_ID;

type CacheResult<T> = Result<T, CacheMiss>;

pub struct Cache(InMemoryCache);

// TODO: Cold resume when fields on CachedGuild & co are pub
impl Cache {
    pub fn new() -> Self {
        let resource_types = ResourceType::CHANNEL
            | ResourceType::GUILD
            | ResourceType::MEMBER
            | ResourceType::ROLE
            | ResourceType::USER_CURRENT;

        let cache = InMemoryCache::builder()
            .message_cache_size(0)
            .resource_types(resource_types)
            .build();

        Self(cache)
    }

    pub fn update(&self, event: &Event) {
        self.0.update(event)
    }

    pub fn stats(&self) -> InMemoryCacheStats<'_> {
        self.0.stats()
    }

    pub fn channel<F, T>(&self, channel: Id<ChannelMarker>, f: F) -> CacheResult<T>
    where
        F: FnOnce(&GuildResource<GuildChannel>) -> T,
    {
        let channel = self
            .0
            .guild_channel(channel)
            .ok_or(CacheMiss::Channel { channel })?;

        Ok(f(&channel))
    }

    pub fn current_user(&self) -> CacheResult<CurrentUser> {
        self.0.current_user().ok_or(CacheMiss::CurrentUser)
    }

    pub fn guild<F, T>(&self, guild: Id<GuildMarker>, f: F) -> CacheResult<T>
    where
        F: FnOnce(&CachedGuild) -> T,
    {
        let guild = self.0.guild(guild).ok_or(CacheMiss::Guild { guild })?;

        Ok(f(&guild))
    }

    pub fn member<F, T>(&self, guild: Id<GuildMarker>, user: Id<UserMarker>, f: F) -> CacheResult<T>
    where
        F: FnOnce(&CachedMember) -> T,
    {
        let member = self
            .0
            .member(guild, user)
            .ok_or(CacheMiss::Member { guild, user })?;

        Ok(f(&member))
    }

    pub fn members<F, T>(&self, guild: Id<GuildMarker>, f: F) -> Vec<T>
    where
        F: Fn(&Id<UserMarker>) -> T,
    {
        self.0
            .guild_members(guild)
            .map_or_else(Vec::new, |entry| entry.iter().map(f).collect())
    }

    pub fn role<F, T>(&self, role: Id<RoleMarker>, f: F) -> CacheResult<T>
    where
        F: FnOnce(&GuildResource<Role>) -> T,
    {
        let role = self.0.role(role).ok_or(CacheMiss::Role { role })?;

        Ok(f(&role))
    }

    pub fn is_guild_owner(
        &self,
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
    ) -> CacheResult<bool> {
        self.guild(guild, |g| g.owner_id() == user)
    }

    pub fn get_guild_permissions(
        &self,
        user: Id<UserMarker>,
        guild: Id<GuildMarker>,
    ) -> (Permissions, RolesLookup) {
        if user.get() == OWNER_USER_ID {
            return (Permissions::all(), RolesLookup::NotChecked);
        }

        match self.is_guild_owner(guild, user) {
            Ok(false) => {}
            Ok(true) => return (Permissions::all(), RolesLookup::NotChecked),
            Err(_) => return (Permissions::empty(), RolesLookup::NotChecked),
        }

        let member_roles = match self.member(guild, user, |m| m.roles().to_owned()) {
            Ok(roles) => roles,
            Err(_) => return (Permissions::empty(), RolesLookup::NotFound),
        };

        let mut permissions = Permissions::empty();

        for &role in &member_roles {
            if let Ok(role_permissions) = self.role(role, |r| r.permissions) {
                if role_permissions.contains(Permissions::ADMINISTRATOR) {
                    return (Permissions::all(), RolesLookup::Found(member_roles));
                }

                permissions |= role_permissions;
            }
        }

        (permissions, RolesLookup::Found(member_roles))
    }

    pub fn get_channel_permissions(
        &self,
        user: Id<UserMarker>,
        channel: Id<ChannelMarker>,
        guild: Id<GuildMarker>,
    ) -> Permissions {
        let (mut permissions, roles) = self.get_guild_permissions(user, guild);

        if permissions.contains(Permissions::ADMINISTRATOR) {
            return Permissions::all();
        }

        if let Ok(Some(permission_overwrites)) = self.channel(channel, |c| match c.deref() {
            GuildChannel::PrivateThread(c) => self.permission_overwrite(c.parent_id),
            GuildChannel::PublicThread(c) => self.permission_overwrite(c.parent_id),
            GuildChannel::Text(c) => Some(c.permission_overwrites.clone()),
            _ => None,
        }) {
            let member_roles = match roles {
                RolesLookup::Found(roles) => Some(roles),
                RolesLookup::NotChecked => self.member(guild, user, |m| m.roles().to_owned()).ok(),
                RolesLookup::NotFound => None,
            };

            if let Some(roles) = member_roles {
                Self::text_channel_permissions(
                    &mut permissions,
                    user,
                    guild,
                    permission_overwrites,
                    roles,
                )
            }
        }

        permissions
    }

    fn text_channel_permissions(
        permissions: &mut Permissions,
        user: Id<UserMarker>,
        guild: Id<GuildMarker>,
        permission_overwrites: Vec<PermissionOverwrite>,
        roles: Vec<Id<RoleMarker>>,
    ) {
        let mut everyone_allowed = Permissions::empty();
        let mut everyone_denied = Permissions::empty();
        let mut user_allowed = Permissions::empty();
        let mut user_denied = Permissions::empty();
        let mut role_allowed = Permissions::empty();
        let mut role_denied = Permissions::empty();

        for overwrite in &permission_overwrites {
            match overwrite.kind {
                PermissionOverwriteType::Member(member) => {
                    if member == user {
                        user_allowed |= overwrite.allow;
                        user_denied |= overwrite.deny;
                    }
                }
                PermissionOverwriteType::Role(role) => {
                    if role.get() == guild.get() {
                        everyone_allowed |= overwrite.allow;
                        everyone_denied |= overwrite.deny
                    } else if roles.contains(&role) {
                        role_allowed |= overwrite.allow;
                        role_denied |= overwrite.deny;
                    }
                }
            }
        }

        *permissions &= !everyone_denied;
        *permissions |= everyone_allowed;

        *permissions &= !role_denied;
        *permissions |= role_allowed;

        *permissions &= !user_denied;
        *permissions |= user_allowed;
    }

    fn permission_overwrite(
        &self,
        channel: Option<Id<ChannelMarker>>,
    ) -> Option<Vec<PermissionOverwrite>> {
        channel.and_then(|channel| {
            self.channel(channel, |c| match c.deref() {
                GuildChannel::Text(c) => Some(c.permission_overwrites.clone()),
                _ => None,
            })
            .ok()
            .flatten()
        })
    }
}

#[derive(Debug, Error)]
pub enum CacheMiss {
    #[error("missing channel {channel}")]
    Channel { channel: Id<ChannelMarker> },
    #[error("missing current user")]
    CurrentUser,
    #[error("missing guild {guild}")]
    Guild { guild: Id<GuildMarker> },
    #[error("missing member {user} in guild {guild}")]
    Member {
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
    },
    #[error("missing role {role}")]
    Role { role: Id<RoleMarker> },
}

pub enum RolesLookup {
    Found(Vec<Id<RoleMarker>>),
    NotChecked,
    NotFound,
}
