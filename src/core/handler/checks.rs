use crate::{
    bail,
    core::{buckets::BucketName, Context},
    BotResult,
};

use std::fmt::Write;
use twilight_model::{channel::Message, guild::Permissions, id::RoleId};

// Is authority -> Ok(None)
// No authority -> Ok(Some(message to user))
// Couldn't figure out -> Err()
pub fn check_authority(ctx: &Context, msg: &Message) -> BotResult<Option<String>> {
    let guild_id = match msg.guild_id {
        Some(id) => id,
        None => return Ok(Some(String::new())),
    };

    let permissions = ctx
        .cache
        .get_guild_permissions_for(msg.author.id, msg.guild_id);

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Ok(None);
    }

    let auth_roles = ctx.config_authorities_collect(guild_id, RoleId);

    if auth_roles.is_empty() {
        let prefix = ctx.config_first_prefix(Some(guild_id));

        let content = format!(
            "You need admin permissions to use this command.\n\
            (`{}help authorities` to adjust authority status for this server)",
            prefix
        );

        return Ok(Some(content));
    } else if let Some(member) = ctx.cache.member(guild_id, msg.author.id) {
        if !member.roles.iter().any(|role| auth_roles.contains(role)) {
            let roles: Vec<_> = auth_roles
                .iter()
                .filter_map(|&role| {
                    ctx.cache.role(role).map_or_else(
                        || {
                            warn!("Role {} not cached for guild {}", role, guild_id);
                            None
                        },
                        |role| Some(role.name.clone()),
                    )
                })
                .collect();

            let role_len: usize = roles.iter().map(|role| role.len()).sum();

            let mut content = String::from(
                "You need either admin permissions or \
                any of these roles to use this command:\n",
            );

            content.reserve_exact(role_len + roles.len().saturating_sub(1) * 4);
            let mut roles = roles.into_iter();

            if let Some(first) = roles.next() {
                content.push_str(&first);

                for role in roles {
                    let _ = write!(content, ", `{}`", role);
                }
            }

            let prefix = ctx.config_first_prefix(Some(guild_id));

            let _ = write!(
                content,
                "\n(`{}help authorities` to adjust authority status for this server)",
                prefix
            );

            return Ok(Some(content));
        }
    } else {
        bail!("member {} not cached for guild {}", msg.author.id, guild_id);
    }

    Ok(None)
}

pub async fn check_ratelimit(
    ctx: &Context,
    msg: &Message,
    bucket: impl AsRef<str>,
) -> Option<(i64, BucketName)> {
    let (ratelimit, bucket) = {
        let bucket: BucketName = bucket.as_ref().parse().unwrap();
        let guard = ctx.buckets.get(&bucket).unwrap();
        let mutex = guard.value();
        let mut bucket_elem = mutex.lock().await;

        match bucket {
            BucketName::Snipe => (bucket_elem.take(0), bucket), // same bucket for everyone
            BucketName::Songs => (
                bucket_elem.take(
                    msg.guild_id
                        .map_or_else(|| msg.author.id.0, |guild_id| guild_id.0), // same bucket for guilds
                ),
                bucket,
            ),
            _ => (bucket_elem.take(msg.author.id.0), bucket),
        }
    };

    if ratelimit > 0 {
        return Some((ratelimit, bucket));
    }

    None
}
