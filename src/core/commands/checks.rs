use std::fmt::Write;

use eyre::Result;
use twilight_model::{
    guild::Permissions,
    id::{
        marker::{GuildMarker, UserMarker},
        Id,
    },
};

use crate::core::{buckets::BucketName, cache::RolesLookup, Context};

/// Is authority -> Ok(None)
/// No authority -> Ok(Some(message to user))
/// Couldn't figure out -> Err()
pub async fn check_authority(
    ctx: &Context,
    author: Id<UserMarker>,
    guild: Option<Id<GuildMarker>>,
) -> Result<Option<String>> {
    let (guild_id, (permissions, roles)) = match guild {
        Some(guild) => (guild, ctx.cache.get_guild_permissions(author, guild)),
        None => return Ok(None),
    };

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Ok(None);
    }

    let auth_roles = ctx.guild_authorities(guild_id).await;

    if auth_roles.is_empty() {
        let content = "You need admin permissions to use this command.\n\
            (`/serverconfig` to adjust authority status for this server)";

        return Ok(Some(content.to_owned()));
    }

    let member_roles = match roles {
        RolesLookup::Found(roles) => roles,
        RolesLookup::NotChecked => ctx
            .cache
            .member(guild_id, author, |member| member.roles().to_owned())?,
        RolesLookup::NotFound => {
            bail!("missing user {author} of guild {guild_id} in cache")
        }
    };

    if !member_roles
        .iter()
        .any(|role| auth_roles.contains(&role.get()))
    {
        let mut content = String::from(
            "You need either admin permissions or \
            any of these roles to use this command:\n",
        );

        content.reserve(auth_roles.len() * 5);
        let mut roles = auth_roles.into_iter();

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

    (ratelimit > 0).then(|| ratelimit)
}
