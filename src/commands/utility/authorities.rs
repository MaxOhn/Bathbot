use std::{borrow::Cow, fmt::Write, sync::Arc};

use twilight_model::{
    guild::Permissions,
    id::{marker::RoleMarker, Id},
};

use crate::{
    util::{
        constants::{GENERAL_ISSUE, OWNER_USER_ID},
        matcher, MessageExt,
    },
    Args, BotResult, CommandData, Context, MessageBuilder,
};

#[command]
#[only_guilds()]
#[authority()]
#[short_desc("Adjust authority roles for a server")]
#[long_desc(
    "Decide which roles should be considered authority roles. \n\
    Authority roles enable the usage of certain commands like \
    `addstream` or `prune`.\n\
    Roles can be given as mention or as role id (up to 10 roles possible).\n\
    If you want to see the current authority roles, just pass \
    `-show` as argument"
)]
#[usage("[@role1] [id of role2] ...")]
#[example("-show", "@Moderator @Mod 83794728403223 @BotCommander")]
#[aliases("authority")]
async fn authorities(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            match AuthorityCommandKind::args(&mut args) {
                Ok(authority_args) => {
                    _authorities(ctx, CommandData::Message { msg, args, num }, authority_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { .. } => unreachable!(),
    }
}

pub(super) async fn _authorities(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: AuthorityCommandKind,
) -> BotResult<()> {
    let guild_id = data.guild_id().unwrap();

    let mut content = match args {
        AuthorityCommandKind::Add(role_id) => {
            let roles = ctx.guild_authorities(guild_id).await;

            if roles.len() >= 10 {
                let content = "You can have at most 10 roles per server setup as authorities.";

                return data.error(&ctx, content).await;
            }

            let update_fut = ctx.update_guild_config(guild_id, move |config| {
                if !config.authorities.contains(&role_id) {
                    config.authorities.push(role_id);
                }
            });

            if let Err(why) = update_fut.await {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }

            "Successfully added authority role. Authority roles now are: ".to_owned()
        }
        AuthorityCommandKind::List => "Current authority roles for this server: ".to_owned(),
        AuthorityCommandKind::Remove(role_id) => {
            let author_id = data.author()?.id;
            let roles = ctx.guild_authorities(guild_id).await;

            if roles.iter().all(|&id| id != role_id) {
                let content = "The role was no authority role anyway";
                let builder = MessageBuilder::new().embed(content);
                data.create_message(&ctx, builder).await?;

                return Ok(());
            }

            // Make sure the author is still an authority after applying new roles
            if !(author_id.get() == OWNER_USER_ID
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

                            return data.error(&ctx, content).await;
                        }
                    }
                    Err(err) => {
                        let _ = data.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err.into());
                    }
                }
            }

            let update_fut = ctx.update_guild_config(guild_id, move |config| {
                config.authorities.retain(|id| *id != role_id);
            });

            if let Err(why) = update_fut.await {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }

            "Successfully removed authority role. Authority roles now are: ".to_owned()
        }
        AuthorityCommandKind::Replace(roles) => {
            let author_id = data.author()?.id;

            // Make sure the author is still an authority after applying new roles
            if !(author_id.get() == OWNER_USER_ID
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

                            return data.error(&ctx, content).await;
                        }
                    }
                    Err(err) => {
                        let _ = data.error(&ctx, GENERAL_ISSUE).await;

                        return Err(err.into());
                    }
                }
            }

            let update_fut = ctx.update_guild_config(guild_id, move |config| {
                config.authorities = roles.into_iter().map(|role| role.get()).collect();
            });

            if let Err(why) = update_fut.await {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }

            "Successfully changed the authority roles to: ".to_owned()
        }
    };

    // Send the message
    let roles = ctx.guild_authorities(guild_id).await;
    role_string(&roles, &mut content);
    let builder = MessageBuilder::new().embed(content);
    data.create_message(&ctx, builder).await?;

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

pub(super) enum AuthorityCommandKind {
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
