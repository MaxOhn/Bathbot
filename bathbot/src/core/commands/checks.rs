use std::fmt::Write;

use bathbot_cache::model::CachedArchive;
use bathbot_model::twilight::{
    channel::{ArchivedPermissionOverwrite, PermissionOverwriteTypeRkyv},
    guild::ArchivedCachedMember,
    id::ArchivedId,
};
use eyre::{ContextCompat, Result};
use rkyv::vec::ArchivedVec;
use twilight_model::{
    channel::permission_overwrite::PermissionOverwriteType,
    guild::Permissions,
    id::{
        Id,
        marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
    },
};

use crate::core::{BotConfig, Context};

/// Is authority -> Ok(None)
/// No authority -> Ok(Some(message to user))
/// Couldn't figure out -> Err()
pub async fn check_authority(
    author: Id<UserMarker>,
    guild: Option<Id<GuildMarker>>,
) -> Result<Option<String>> {
    let (guild_id, (permissions, roles)) = match guild {
        Some(guild) => (guild, check_guild_permissions(author, guild).await),
        None => return Ok(None),
    };

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Ok(None);
    }

    let auth_roles = Context::guild_config()
        .peek(guild_id, |config| config.authorities.clone())
        .await;

    if auth_roles.is_empty() {
        let content = "You need admin permissions to use this command.\n\
            (`/serverconfig` to adjust authority status for this server)";

        return Ok(Some(content.to_owned()));
    }

    let member = match roles {
        RolesLookup::Found(member) => member,
        RolesLookup::NotChecked => Context::cache()
            .member(guild_id, author)
            .await?
            .wrap_err("Missing member in cache")?,
        RolesLookup::NotFound => {
            bail!("Missing user {author} of guild {guild_id} in cache")
        }
    };

    if !member
        .roles
        .iter()
        .any(|role| auth_roles.contains(&Id::from(*role)))
    {
        let mut content = String::from(
            "You need either admin permissions or \
            any of these roles to use this command:\n",
        );

        content.reserve(auth_roles.len() * 5);
        let mut roles = auth_roles.iter();

        if let Some(first) = roles.next() {
            let _ = write!(content, "<@&{first}>");

            for role in roles {
                let _ = write!(content, ", <@&{role}>");
            }
        }

        content.push_str("\n(`/serverconfig` to adjust authority status for this server)");

        return Ok(Some(content));
    }

    Ok(None)
}

pub async fn check_guild_permissions(
    user: Id<UserMarker>,
    guild: Id<GuildMarker>,
) -> (Permissions, RolesLookup) {
    if user == BotConfig::get().owner {
        return (Permissions::all(), RolesLookup::NotChecked);
    }

    let cache = Context::cache();

    match cache.guild(guild).await {
        Ok(Some(guild)) if guild.owner_id == user => {
            return (Permissions::all(), RolesLookup::NotChecked);
        }
        Ok(Some(_)) => {}
        Ok(None) => return (Permissions::empty(), RolesLookup::NotChecked),
        Err(err) => {
            warn!("{err:?}");

            return (Permissions::empty(), RolesLookup::NotChecked);
        }
    }

    let member = match cache.member(guild, user).await {
        Ok(Some(member)) => member,
        Ok(None) => return (Permissions::empty(), RolesLookup::NotFound),
        Err(err) => {
            warn!("{err:?}");

            return (Permissions::empty(), RolesLookup::NotFound);
        }
    };

    let mut permissions = Permissions::empty();

    for role in member.roles.iter() {
        if let Ok(Some(role)) = cache.role(guild, role.to_native()).await {
            let role_permissions = Permissions::from_bits_truncate(role.permissions.to_native());

            if role_permissions.contains(Permissions::ADMINISTRATOR) {
                return (Permissions::all(), RolesLookup::Found(member));
            }

            permissions |= role_permissions;
        }
    }

    (permissions, RolesLookup::Found(member))
}

pub async fn check_channel_permissions(
    user: Id<UserMarker>,
    channel: Id<ChannelMarker>,
    guild: Id<GuildMarker>,
) -> Permissions {
    let (mut permissions, roles) = check_guild_permissions(user, guild).await;

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Permissions::all();
    }

    let cache = Context::cache();

    if let Ok(Some(channel)) = cache.channel(Some(guild), channel).await
        && let Some(permission_overwrites) = channel.permission_overwrites.as_ref()
    {
        let member = match roles {
            RolesLookup::Found(roles) => Some(roles),
            RolesLookup::NotChecked => cache.member(guild, user).await.ok().flatten(),
            RolesLookup::NotFound => None,
        };

        if let Some(member) = member {
            text_channel_permissions(
                &mut permissions,
                user,
                guild,
                permission_overwrites,
                member.roles.as_slice(),
            )
        }
    }

    permissions
}

fn text_channel_permissions(
    permissions: &mut Permissions,
    user: Id<UserMarker>,
    guild: Id<GuildMarker>,
    permission_overwrites: &ArchivedVec<ArchivedPermissionOverwrite>,
    roles: &[ArchivedId<RoleMarker>],
) {
    let mut everyone_allowed = Permissions::empty();
    let mut everyone_denied = Permissions::empty();
    let mut user_allowed = Permissions::empty();
    let mut user_denied = Permissions::empty();
    let mut role_allowed = Permissions::empty();
    let mut role_denied = Permissions::empty();

    for overwrite in permission_overwrites.iter() {
        match PermissionOverwriteTypeRkyv::deserialize(overwrite.kind) {
            PermissionOverwriteType::Member => {
                if overwrite.id.cast() == user {
                    user_allowed |= Permissions::from_bits_truncate(overwrite.allow.to_native());
                    user_denied |= Permissions::from_bits_truncate(overwrite.deny.to_native());
                }
            }
            PermissionOverwriteType::Role => {
                if overwrite.id.cast() == guild {
                    everyone_allowed |=
                        Permissions::from_bits_truncate(overwrite.allow.to_native());
                    everyone_denied |= Permissions::from_bits_truncate(overwrite.deny.to_native());
                } else if roles.contains(&overwrite.id.cast()) {
                    role_allowed |= Permissions::from_bits_truncate(overwrite.allow.to_native());
                    role_denied |= Permissions::from_bits_truncate(overwrite.deny.to_native());
                }
            }
            _ => {}
        }
    }

    *permissions &= !everyone_denied;
    *permissions |= everyone_allowed;

    *permissions &= !role_denied;
    *permissions |= role_allowed;

    *permissions &= !user_denied;
    *permissions |= user_allowed;
}

pub enum RolesLookup {
    Found(CachedArchive<ArchivedCachedMember>),
    NotChecked,
    NotFound,
}
