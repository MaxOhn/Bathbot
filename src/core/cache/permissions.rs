use twilight_model::{
    channel::permission_overwrite::{PermissionOverwrite, PermissionOverwriteType},
    guild::Permissions,
    id::{
        marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
        Id,
    },
};

use crate::util::constants::OWNER_USER_ID;

use super::Cache;

impl Cache {
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

        if let Ok(Some(permission_overwrites)) =
            self.channel(channel, |c| c.permission_overwrites.clone())
        {
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
                PermissionOverwriteType::Member => {
                    if overwrite.id.cast() == user {
                        user_allowed |= overwrite.allow;
                        user_denied |= overwrite.deny;
                    }
                }
                PermissionOverwriteType::Role => {
                    if overwrite.id.cast() == guild {
                        everyone_allowed |= overwrite.allow;
                        everyone_denied |= overwrite.deny
                    } else if roles.contains(&overwrite.id.cast()) {
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
}

pub enum RolesLookup {
    Found(Vec<Id<RoleMarker>>),
    NotChecked,
    NotFound,
}
