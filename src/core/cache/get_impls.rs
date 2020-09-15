use super::{Cache, CachedChannel, CachedGuild, CachedMember, CachedRole, CachedUser};
use crate::util::constants::OWNER_USER_ID;

use std::sync::Arc;
use twilight_model::{
    channel::permission_overwrite::PermissionOverwriteType,
    guild::Permissions,
    id::{ChannelId, GuildId, RoleId, UserId},
    user::User,
};

impl Cache {
    pub fn is_guild_owner(&self, guild_id: GuildId, user_id: UserId) -> bool {
        self.guilds
            .get(&guild_id)
            .map(|guard| guard.value().owner_id == user_id)
            .unwrap_or(false)
    }

    pub fn get_guild_permissions_for(
        &self,
        user_id: UserId,
        guild_id: Option<GuildId>,
    ) -> Permissions {
        if user_id.0 == OWNER_USER_ID {
            return Permissions::all();
        }
        let guild = match guild_id.and_then(|id| self.get_guild(id)) {
            Some(guild) => guild,
            None => return Permissions::empty(),
        };
        if guild.owner_id == user_id {
            return Permissions::all();
        }
        let member = match self.get_member(user_id, guild.id) {
            Some(member) => member,
            None => return Permissions::empty(),
        };
        let mut permissions = Permissions::empty();
        for &role_id in &member.roles {
            if let Some(role) = guild.get_role(role_id) {
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
        let mut permissions = Permissions::empty();
        if let Some(channel) = self.get_channel(channel_id) {
            if channel.is_dm() {
                return Permissions::SEND_MESSAGES
                    | Permissions::EMBED_LINKS
                    | Permissions::ATTACH_FILES
                    | Permissions::USE_EXTERNAL_EMOJIS
                    | Permissions::ADD_REACTIONS
                    | Permissions::READ_MESSAGE_HISTORY;
            }
            permissions = self.get_guild_permissions_for(user_id, guild_id);
            if permissions.contains(Permissions::ADMINISTRATOR) {
                return Permissions::all();
            }
            if let Some(member) = guild_id.and_then(|id| self.get_member(user_id, id)) {
                let overrides = channel.get_permission_overrides();
                let mut everyone_allowed = Permissions::empty();
                let mut everyone_denied = Permissions::empty();
                let mut user_allowed = Permissions::empty();
                let mut user_denied = Permissions::empty();
                let mut role_allowed = Permissions::empty();
                let mut role_denied = Permissions::empty();
                for o in overrides {
                    match o.kind {
                        PermissionOverwriteType::Member(member_id) => {
                            if member_id == user_id {
                                user_allowed |= o.allow;
                                user_denied |= o.deny;
                            }
                        }
                        PermissionOverwriteType::Role(role_id) => {
                            if role_id.0 == channel.get_guild_id().unwrap().0 {
                                everyone_allowed |= o.allow;
                                everyone_denied |= o.deny
                            } else if member.roles.contains(&role_id) {
                                role_allowed |= o.allow;
                                role_denied |= o.deny;
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
        }
        permissions
    }

    pub fn get_guild(&self, guild_id: GuildId) -> Option<Arc<CachedGuild>> {
        self.guilds
            .get(&guild_id)
            .map(|guard| guard.value().clone())
    }

    pub fn get_channel(&self, channel_id: ChannelId) -> Option<Arc<CachedChannel>> {
        self.guild_channels
            .get(&channel_id)
            .or_else(|| self.private_channels.get(&channel_id))
            .map(|guard| guard.value().clone())
    }

    pub fn get_user(&self, user_id: UserId) -> Option<Arc<CachedUser>> {
        self.users.get(&user_id).map(|guard| guard.value().clone())
    }

    pub fn get_or_insert_user(&self, user: &User) -> Arc<CachedUser> {
        match self.get_user(user.id) {
            Some(user) => user,
            None => {
                let arc = Arc::new(CachedUser::from_user(user));
                self.users.insert(arc.id, arc.clone());
                self.stats.user_counts.unique.inc();
                arc
            }
        }
    }

    pub fn get_role(&self, role_id: RoleId, guild_id: GuildId) -> Option<Arc<CachedRole>> {
        self.guilds
            .get(&guild_id)
            .and_then(|guard| guard.value().get_role(role_id))
    }

    pub fn get_guild_channel(
        &self,
        channel_id: ChannelId,
        guild_id: GuildId,
    ) -> Option<Arc<CachedChannel>> {
        self.guilds
            .get(&guild_id)
            .and_then(|guard| Some(guard.value().channels.get(&channel_id)?.value().clone()))
    }

    pub fn get_member(&self, user_id: UserId, guild_id: GuildId) -> Option<Arc<CachedMember>> {
        self.guilds
            .get(&guild_id)
            .and_then(|guard| Some(guard.value().members.get(&user_id)?.value().clone()))
    }
}
