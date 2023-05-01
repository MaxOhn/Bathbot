use std::fmt::Write;

use bathbot_cache::{model::CachedArchive, Cache};
use bathbot_model::twilight_model::{
    channel::{PermissionOverwrite, PermissionOverwriteTypeRkyv},
    guild::Member,
};
use eyre::{ContextCompat, Result};
use rkyv::{with::DeserializeWith, Archived, Infallible};
use twilight_model::{
    channel::permission_overwrite::PermissionOverwriteType,
    guild::Permissions,
    id::{
        marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
        Id,
    },
};

use crate::core::{buckets::BucketName, BotConfig, Context};

/// Is authority -> Ok(None)
/// No authority -> Ok(Some(message to user))
/// Couldn't figure out -> Err()
pub async fn check_authority(
    ctx: &Context,
    author: Id<UserMarker>,
    guild: Option<Id<GuildMarker>>,
) -> Result<Option<String>> {
    let (guild_id, (permissions, roles)) = match guild {
        Some(guild) => (
            guild,
            check_guild_permissions(&ctx.cache, author, guild).await,
        ),
        None => return Ok(None),
    };

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Ok(None);
    }

    let auth_roles = ctx
        .guild_config()
        .peek(guild_id, |config| config.authorities.clone())
        .await;

    if auth_roles.is_empty() {
        let content = "You need admin permissions to use this command.\n\
            (`/serverconfig` to adjust authority status for this server)";

        return Ok(Some(content.to_owned()));
    }

    let member = match roles {
        RolesLookup::Found(member) => member,
        RolesLookup::NotChecked => ctx
            .cache
            .member(guild_id, author)
            .await?
            .wrap_err("Missing member in cache")?,
        RolesLookup::NotFound => {
            bail!("Missing user {author} of guild {guild_id} in cache")
        }
    };

    if !member.roles().iter().any(|role| auth_roles.contains(role)) {
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

pub async fn check_ratelimit(
    ctx: &Context,
    user: Id<UserMarker>,
    bucket: BucketName,
) -> Option<i64> {
    let ratelimit = ctx.buckets.get(bucket).lock().take(user.get());

    (ratelimit > 0).then_some(ratelimit)
}

pub async fn check_guild_permissions(
    cache: &Cache,
    user: Id<UserMarker>,
    guild: Id<GuildMarker>,
) -> (Permissions, RolesLookup) {
    if user == BotConfig::get().owner {
        return (Permissions::all(), RolesLookup::NotChecked);
    }

    match cache.guild(guild).await {
        Ok(Some(guild)) if guild.owner_id == user => {
            return (Permissions::all(), RolesLookup::NotChecked)
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

    for &role in member.roles().iter() {
        if let Ok(Some(role)) = cache.role(guild, role).await {
            if role.permissions.contains(Permissions::ADMINISTRATOR) {
                return (Permissions::all(), RolesLookup::Found(member));
            }

            permissions |= role.permissions;
        }
    }

    (permissions, RolesLookup::Found(member))
}

pub async fn check_channel_permissions(
    cache: &Cache,
    user: Id<UserMarker>,
    channel: Id<ChannelMarker>,
    guild: Id<GuildMarker>,
) -> Permissions {
    let (mut permissions, roles) = check_guild_permissions(cache, user, guild).await;

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Permissions::all();
    }

    if let Ok(Some(channel)) = cache.channel(Some(guild), channel).await {
        if let Some(permission_overwrites) = channel.permission_overwrites.as_ref() {
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
                    member.roles(),
                )
            }
        }
    }

    permissions
}

fn text_channel_permissions(
    permissions: &mut Permissions,
    user: Id<UserMarker>,
    guild: Id<GuildMarker>,
    permission_overwrites: &Archived<Box<[PermissionOverwrite]>>,
    roles: &[Id<RoleMarker>],
) {
    let mut everyone_allowed = Permissions::empty();
    let mut everyone_denied = Permissions::empty();
    let mut user_allowed = Permissions::empty();
    let mut user_denied = Permissions::empty();
    let mut role_allowed = Permissions::empty();
    let mut role_denied = Permissions::empty();

    for overwrite in permission_overwrites.iter() {
        match PermissionOverwriteTypeRkyv::deserialize_with(&overwrite.kind, &mut Infallible)
            .unwrap()
        {
            PermissionOverwriteType::Member => {
                if overwrite.id.cast() == user {
                    user_allowed |= overwrite.allow;
                    user_denied |= overwrite.deny;
                }
            }
            PermissionOverwriteType::Role => {
                if overwrite.id.cast() == guild {
                    everyone_allowed |= overwrite.allow;
                    everyone_denied |= overwrite.deny;
                } else if roles.contains(&overwrite.id.cast()) {
                    role_allowed |= overwrite.allow;
                    role_denied |= overwrite.deny;
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
    Found(CachedArchive<Member>),
    NotChecked,
    NotFound,
}
