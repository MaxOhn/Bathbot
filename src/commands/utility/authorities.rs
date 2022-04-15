use std::{borrow::Cow, fmt::Write, sync::Arc};

use command_macros::command;
use twilight_model::{
    guild::Permissions,
    id::{marker::RoleMarker, Id},
};

use crate::{
    core::{
        commands::{prefix::Args, CommandOrigin},
        Context, CONFIG,
    },
    util::{builder::MessageBuilder, constants::GENERAL_ISSUE, matcher, ChannelExt},
    BotResult,
};

#[command]
#[desc("Adjust authority roles for a server")]
#[help(
    "Decide which roles should be considered authority roles. \n\
    Authority roles enable the usage of certain commands like \
    `addstream` or `prune`.\n\
    Roles can be given as mention or as role id (up to 10 roles possible).\n\
    If you want to see the current authority roles, just pass \
    `-show` as argument"
)]
#[usage("[@role1] [id of role2] ...")]
#[example("-show", "@Moderator @Mod 83794728403223 @BotCommander")]
#[alias("authority")]
#[flags(AUTHORITY, ONLY_GUILDS, SKIP_DEFER)]
#[group(Utility)]
async fn prefix_authorities(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> BotResult<()> {
    match AuthorityCommandKind::args(&mut args) {
        Ok(args) => authorities(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

pub async fn authorities(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: AuthorityCommandKind,
) -> BotResult<()> {
    let guild_id = orig.guild_id().unwrap();

    let mut content = match args {
        AuthorityCommandKind::Add(role_id) => {
            let roles = ctx.guild_authorities(guild_id).await;

            if roles.len() >= 10 {
                let content = "You can have at most 10 roles per server setup as authorities.";

                return orig.error_callback(&ctx, content).await;
            }

            let update_fut = ctx.update_guild_config(guild_id, move |config| {
                if !config.authorities.contains(&role_id) {
                    config.authorities.push(role_id);
                }
            });

            if let Err(why) = update_fut.await {
                let _ = orig.error_callback(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }

            "Successfully added authority role. Authority roles now are: ".to_owned()
        }
        AuthorityCommandKind::List => "Current authority roles for this server: ".to_owned(),
        AuthorityCommandKind::Remove(role_id) => {
            let author_id = orig.user_id()?;
            let roles = ctx.guild_authorities(guild_id).await;

            if roles.iter().all(|&id| id != role_id) {
                let content = "The role was no authority role anyway";
                let builder = MessageBuilder::new().embed(content);
                orig.callback(&ctx, builder).await?;

                return Ok(());
            }

            // Make sure the author is still an authority after applying new roles
            if !(author_id == CONFIG.get().unwrap().owner
                || ctx
                    .cache
                    .is_guild_owner(guild_id, author_id)
                    .unwrap_or(false))
            {
                match ctx
                    .cache
                    .member(guild_id, author_id, |member| member.roles().to_owned())
                {
                    Ok(member_roles) => {
                        let still_authority = member_roles
                            .into_iter()
                            .map(|role| ctx.cache.role(role, |role| (role.id, role.permissions)))
                            .any(|role_result| match role_result {
                                Ok((id, permissions)) => {
                                    permissions.contains(Permissions::ADMINISTRATOR)
                                        || roles
                                            .iter()
                                            .any(|&new| new == id.get() && new != role_id)
                                }
                                _ => false,
                            });

                        if !still_authority {
                            let content = "You cannot set authority roles to something \
                                that would make you lose authority status.";

                            return orig.error_callback(&ctx, content).await;
                        }
                    }
                    Err(err) => {
                        let _ = orig.error_callback(&ctx, GENERAL_ISSUE).await;

                        return Err(err.into());
                    }
                }
            }

            let update_fut = ctx.update_guild_config(guild_id, move |config| {
                config.authorities.retain(|id| *id != role_id);
            });

            if let Err(err) = update_fut.await {
                let _ = orig.error_callback(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }

            "Successfully removed authority role. Authority roles now are: ".to_owned()
        }
        AuthorityCommandKind::Replace(roles) => {
            let author_id = orig.user_id()?;

            // Make sure the author is still an authority after applying new roles
            if !(author_id == CONFIG.get().unwrap().owner
                || ctx
                    .cache
                    .is_guild_owner(guild_id, author_id)
                    .unwrap_or(false))
            {
                match ctx
                    .cache
                    .member(guild_id, author_id, |member| member.roles().to_owned())
                {
                    Ok(member_roles) => {
                        let still_authority = member_roles
                            .into_iter()
                            .map(|role| ctx.cache.role(role, |role| (role.id, role.permissions)))
                            .any(|role_result| match role_result {
                                Ok((id, permissions)) => {
                                    permissions.contains(Permissions::ADMINISTRATOR)
                                        || roles.iter().any(|&new| new == id)
                                }
                                _ => false,
                            });

                        if !still_authority {
                            let content = "You cannot set authority roles to something \
                                that would make you lose authority status.";

                            return orig.error_callback(&ctx, content).await;
                        }
                    }
                    Err(err) => {
                        let _ = orig.error_callback(&ctx, GENERAL_ISSUE).await;

                        return Err(err.into());
                    }
                }
            }

            let update_fut = ctx.update_guild_config(guild_id, move |config| {
                config.authorities = roles.into_iter().map(|role| role.get()).collect();
            });

            if let Err(err) = update_fut.await {
                let _ = orig.error_callback(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }

            "Successfully changed the authority roles to: ".to_owned()
        }
    };

    // Send the message
    let roles = ctx.guild_authorities(guild_id).await;
    role_string(&roles, &mut content);
    let builder = MessageBuilder::new().embed(content);
    orig.callback(&ctx, builder).await?;

    Ok(())
}

fn role_string(roles: &[u64], content: &mut String) {
    let mut iter = roles.iter();

    if let Some(first) = iter.next() {
        content.reserve(roles.len() * 20);
        let _ = write!(content, "<@&{first}>");

        for role in iter {
            let _ = write!(content, ", <@&{role}>");
        }
    } else {
        content.push_str("None");
    }
}

pub enum AuthorityCommandKind {
    Add(u64),
    List,
    Remove(u64),
    Replace(Vec<Id<RoleMarker>>),
}

fn parse_role(arg: &str) -> Result<Id<RoleMarker>, Cow<'static, str>> {
    matcher::get_mention_role(arg)
        .ok_or_else(|| format!("Expected role mention or role id, got `{arg}`").into())
}

impl AuthorityCommandKind {
    fn args(args: &mut Args<'_>) -> Result<Self, String> {
        let mut roles = match args.next() {
            Some("-show") | Some("show") => return Ok(Self::List),
            Some(arg) => vec![parse_role(arg)?],
            None => return Ok(Self::Replace(Vec::new())),
        };

        for arg in args.take(9) {
            roles.push(parse_role(arg)?);
        }

        Ok(Self::Replace(roles))
    }
}
