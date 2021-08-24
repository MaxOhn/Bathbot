mod command;
mod data;
mod group;
mod handle_message;
mod handle_slash;
pub mod parse;

pub use command::Command;
pub use data::{CommandData, CommandDataCompact};
pub use group::{CommandGroup, CommandGroups, CMD_GROUPS};
pub use handle_message::handle_message;
pub use handle_slash::handle_interaction;
pub use parse::Invoke;

use crate::{bail, core::buckets::BucketName, util::Authored, BotResult, Context, Error};

use std::fmt::{Display, Formatter, Result as FmtResult, Write};
use twilight_model::{
    guild::Permissions,
    id::{GuildId, RoleId, UserId},
};

#[derive(Debug)]
enum ProcessResult {
    Success,
    NoDM,
    NoSendPermission,
    Ratelimited(BucketName),
    NoOwner,
    NoAuthority,
}

impl ProcessResult {
    fn success(_: ()) -> Self {
        Self::Success
    }
}

impl Display for ProcessResult {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Self::Ratelimited(bucket) => write!(f, "Ratelimited ({:?})", bucket),
            _ => write!(f, "{:?}", self),
        }
    }
}

// Is authority -> Ok(None)
// No authority -> Ok(Some(message to user))
// Couldn't figure out -> Err()
async fn check_authority(ctx: &Context, authored: &impl Authored) -> BotResult<Option<String>> {
    let author_id = authored.author().ok_or(Error::MissingSlashAuthor)?.id;
    let guild_id = authored.guild_id();

    _check_authority(ctx, author_id, guild_id).await
}

async fn _check_authority(
    ctx: &Context,
    author_id: UserId,
    guild_id: Option<GuildId>,
) -> BotResult<Option<String>> {
    let (guild_id, permissions) = match guild_id {
        Some(id) => (id, ctx.cache.get_guild_permissions_for(author_id, guild_id)),
        None => return Ok(Some(String::new())),
    };

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Ok(None);
    }

    let auth_roles = ctx.config_authorities_collect(guild_id, RoleId).await;

    if auth_roles.is_empty() {
        let prefix = ctx.config_first_prefix(Some(guild_id)).await;

        let content = format!(
            "You need admin permissions to use this command.\n\
            (`{}help authorities` to adjust authority status for this server)",
            prefix
        );

        return Ok(Some(content));
    } else if let Some(member) = ctx.cache.member(guild_id, author_id) {
        if !member.roles.iter().any(|role| auth_roles.contains(role)) {
            let mut content = String::from(
                "You need either admin permissions or \
                any of these roles to use this command:\n",
            );

            content.reserve(auth_roles.len() * 5);
            let mut roles = auth_roles.into_iter();

            if let Some(first) = roles.next() {
                let _ = write!(content, "<@&{}>", first);

                for role in roles {
                    let _ = write!(content, ", <@&{}>", role);
                }
            }

            let prefix = ctx.config_first_prefix(Some(guild_id)).await;

            let _ = write!(
                content,
                "\n(`{}help authorities` to adjust authority status for this server)",
                prefix
            );

            return Ok(Some(content));
        }
    } else {
        bail!("member {} not cached for guild {}", author_id, guild_id);
    }

    Ok(None)
}

async fn check_ratelimit(
    ctx: &Context,
    authored: &impl Authored,
    bucket: impl AsRef<str>,
) -> Option<(i64, BucketName)> {
    // * Note: Dangerous `?` if author is None but ratelimit should apply.
    // * Should be caught elsewhere though so this is likely fine
    let author_id = authored.author()?.id;
    let guild_id = authored.guild_id();

    _check_ratelimit(ctx, author_id, guild_id, bucket.as_ref().parse().unwrap()).await
}

async fn _check_ratelimit(
    ctx: &Context,
    author_id: UserId,
    guild_id: Option<GuildId>,
    bucket: BucketName,
) -> Option<(i64, BucketName)> {
    let (ratelimit, bucket) = {
        let guard = ctx.buckets.get(&bucket).unwrap();
        let mutex = guard.value();
        let mut bucket_elem = mutex.lock().await;

        match bucket {
            BucketName::Snipe => (bucket_elem.take(0), bucket), // same bucket for everyone
            BucketName::Songs => {
                let id = guild_id.map_or_else(|| author_id.0, |guild_id| guild_id.0); // same bucket for guilds

                (bucket_elem.take(id), bucket)
            }
            _ => (bucket_elem.take(author_id.0), bucket),
        }
    };

    if ratelimit > 0 {
        return Some((ratelimit, bucket));
    }

    None
}
