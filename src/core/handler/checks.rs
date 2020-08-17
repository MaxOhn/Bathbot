use crate::{
    bail,
    core::{buckets::BucketName, Context},
    BotResult,
};

use rayon::prelude::*;
use std::fmt::Write;
use twilight::model::{channel::Message, guild::Permissions, id::RoleId};

// Is authority -> Ok(None)
// No authority -> Ok(Some(message to user))
// Couldn't figure out -> Err()
pub fn check_authority(ctx: &Context, msg: &Message) -> BotResult<Option<String>> {
    let guild_id = match msg.guild_id {
        Some(id) => id,
        None => return Ok(Some(String::new()))
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
    } else if let Some(member) = ctx.cache.get_member(msg.author.id, guild_id) {
        if !member
            .roles
            .par_iter()
            .any(|role| auth_roles.contains(role))
        {
            let roles: Vec<_> = auth_roles
                .par_iter()
                .filter_map(|&role| {
                    ctx.cache.get_role(role, guild_id).map_or_else(
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
            content.reserve_exact(role_len + (roles.len() - 1) * 2);
            let mut roles = roles.into_iter();
            content.push_str(&roles.next().unwrap());
            for role in roles {
                let _ = write!(content, ", {}", role);
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
    bucket: impl Into<BucketName>,
) -> Option<i64> {
    let rate_limit = {
        let bucket = bucket.into();
        let guard = ctx.buckets.get(&bucket).unwrap();
        let mutex = guard.value();
        let mut bucket = mutex.lock().await;
        bucket.take(msg.author.id.0)
    };
    if rate_limit > 0 {
        return Some(rate_limit);
    }
    None
}
