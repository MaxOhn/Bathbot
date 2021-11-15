use std::{borrow::Cow, fmt::Write, sync::Arc};

use bathbot_cache::model::{CachedRole, GuildOrId};
use eyre::Report;
use futures::{future, stream::FuturesUnordered, StreamExt};
use twilight_model::{
    application::interaction::{application_command::CommandOptionValue, ApplicationCommand},
    guild::Permissions,
};

use crate::{
    commands::{MyCommand, MyCommandOption},
    util::{
        constants::{GENERAL_ISSUE, OWNER_USER_ID},
        matcher, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, MessageBuilder,
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
            match AuthorityCommandKind::args(&ctx, &mut args).await {
                Ok(authority_args) => {
                    _authorities(ctx, CommandData::Message { msg, args, num }, authority_args).await
                }
                Err(content) => msg.error(&ctx, content).await,
            }
        }
        CommandData::Interaction { command } => slash_authorities(ctx, *command).await,
    }
}

async fn _authorities(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: AuthorityCommandKind,
) -> BotResult<()> {
    let guild_id = data.guild_id().unwrap();

    let mut content = match args {
        AuthorityCommandKind::Add(role_id) => {
            let roles = ctx.config_authorities(guild_id).await;

            if roles.len() >= 10 {
                let content = "You can have at most 10 roles per server setup as authorities.";

                return data.error(&ctx, content).await;
            }

            let update_fut = ctx.update_guild_config(guild_id, move |config| {
                config.authorities.push(role_id);
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
            let roles = ctx.config_authorities(guild_id).await;

            if roles.iter().all(|&id| id != role_id) {
                let content = "The role was no authority role anyway";
                let builder = MessageBuilder::new().embed(content);
                data.create_message(&ctx, builder).await?;

                return Ok(());
            }

            let guild_id_ = GuildOrId::Id(guild_id);
            let is_owner_fut = ctx.cache.is_guild_owner(&guild_id_, author_id);

            // Make sure the author is still an authority after applying new roles
            if !(author_id.get() == OWNER_USER_ID || matches!(is_owner_fut.await, Ok(true))) {
                match ctx.cache.member(guild_id, author_id).await {
                    Ok(Some(member)) => {
                        let still_authority = member
                            .roles
                            .iter()
                            .map(|&role| ctx.cache.role(role))
                            .collect::<FuturesUnordered<_>>()
                            .any(|role_result| {
                                let result = match role_result {
                                    Ok(Some(role)) => {
                                        role.permissions.contains(Permissions::ADMINISTRATOR)
                                            || roles.iter().any(|&new| new == role.id.get())
                                    }
                                    _ => false,
                                };

                                future::ready(result)
                            })
                            .await;

                        if !still_authority {
                            let content = "You cannot set authority roles to something \
                                that would make you lose authority status.";

                            return data.error(&ctx, content).await;
                        }
                    }
                    Ok(None) => {
                        let _ = data.error(&ctx, GENERAL_ISSUE).await;

                        bail!("member {} not cached for guild {}", author_id, guild_id);
                    }
                    Err(why) => {
                        let _ = data.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why.into());
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
            let guild_id_ = GuildOrId::Id(guild_id);
            let is_owner_fut = ctx.cache.is_guild_owner(&guild_id_, author_id);

            // Make sure the author is still an authority after applying new roles
            if !(author_id.get() == OWNER_USER_ID || matches!(is_owner_fut.await, Ok(true))) {
                match ctx.cache.member(guild_id, author_id).await {
                    Ok(Some(member)) => {
                        let still_authority = member
                            .roles
                            .iter()
                            .map(|&role| ctx.cache.role(role))
                            .collect::<FuturesUnordered<_>>()
                            .any(|role_result| {
                                let result = match role_result {
                                    Ok(Some(role)) => {
                                        role.permissions.contains(Permissions::ADMINISTRATOR)
                                            || roles.iter().any(|new| new.id == role.id)
                                    }
                                    _ => false,
                                };

                                future::ready(result)
                            })
                            .await;

                        if !still_authority {
                            let content = "You cannot set authority roles to something \
                                that would make you lose authority status.";

                            return data.error(&ctx, content).await;
                        }
                    }
                    Ok(None) => {
                        let _ = data.error(&ctx, GENERAL_ISSUE).await;

                        bail!("member {} not cached for guild {}", author_id, guild_id);
                    }

                    Err(why) => {
                        let _ = data.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why.into());
                    }
                }
            }

            let update_fut = ctx.update_guild_config(guild_id, move |config| {
                config.authorities = roles.into_iter().map(|role| role.id.get()).collect();
            });

            if let Err(why) = update_fut.await {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }

            "Successfully changed the authority roles to: ".to_owned()
        }
    };

    // Send the message
    let roles = ctx.config_authorities(guild_id).await;
    role_string(&roles, &mut content);
    let builder = MessageBuilder::new().embed(content);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

fn role_string(roles: &[u64], content: &mut String) {
    let mut iter = roles.iter();

    if let Some(first) = iter.next() {
        content.reserve(roles.len() * 20);
        let _ = write!(content, "<@&{}>", first);

        for role in iter {
            let _ = write!(content, ", <@&{}>", role);
        }
    } else {
        content.push_str("None");
    }
}

enum AuthorityCommandKind {
    Add(u64),
    List,
    Remove(u64),
    Replace(Vec<CachedRole>),
}

async fn parse_role(ctx: &Context, arg: &str) -> Result<CachedRole, Cow<'static, str>> {
    let role_id = match matcher::get_mention_role(arg) {
        Some(id) => id,
        None => return Err(format!("Expected role mention or role id, got `{}`", arg).into()),
    };

    match ctx.cache.role(role_id).await {
        Ok(Some(role)) => Ok(role),
        Ok(None) => Err(format!("No role with id {} found in this guild", role_id).into()),
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to retrieve role from cache");
            warn!("{:?}", report);

            Err(GENERAL_ISSUE.into())
        }
    }
}

impl AuthorityCommandKind {
    async fn args(ctx: &Context, args: &mut Args<'_>) -> Result<Self, String> {
        let mut roles = match args.next() {
            Some("-show") | Some("show") => return Ok(Self::List),
            Some(arg) => vec![parse_role(ctx, arg).await?],
            None => return Ok(Self::Replace(Vec::new())),
        };

        for arg in args.take(9) {
            roles.push(parse_role(ctx, arg).await?);
        }

        Ok(Self::Replace(roles))
    }

    fn slash(command: &ApplicationCommand) -> BotResult<Self> {
        command
            .data
            .options
            .first()
            .and_then(|option| match &option.value {
                CommandOptionValue::SubCommand(options) => match option.name.as_str() {
                    "add" => {
                        let role = options.first().and_then(|option| match option.value {
                            CommandOptionValue::Role(value) => Some(value),
                            _ => None,
                        })?;

                        Some(AuthorityCommandKind::Add(role.get()))
                    }
                    "list" => Some(AuthorityCommandKind::List),
                    "remove" => {
                        let role = options.first().and_then(|option| match option.value {
                            CommandOptionValue::Role(value) => Some(value),
                            _ => None,
                        })?;

                        Some(AuthorityCommandKind::Remove(role.get()))
                    }
                    _ => None,
                },
                _ => None,
            })
            .ok_or(Error::InvalidCommandOptions)
    }
}

pub async fn slash_authorities(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    let args = AuthorityCommandKind::slash(&command)?;

    _authorities(ctx, command.into(), args).await
}

pub fn define_authorities() -> MyCommand {
    let role =
        MyCommandOption::builder("role", "Specify the role that should gain authority status")
            .role(true);

    let add = MyCommandOption::builder("add", "Add authority status to a role")
        .help("Add authority status to a role.\nServers can have at most 10 authority roles.")
        .subcommand(vec![role]);

    let list = MyCommandOption::builder("list", "Display all current authority roles")
        .subcommand(Vec::new());

    let role =
        MyCommandOption::builder("role", "Specify the role that should lose authority status")
            .role(true);

    let remove_help = "Remove authority status from a role.\n\
        You can only use this if the removed role would __not__ make you lose authority status yourself.";

    let remove = MyCommandOption::builder("remove", "Remove authority status from a role")
        .help(remove_help)
        .subcommand(vec![role]);

    let help = "To use certain commands, users require a special status.\n\
        This command adjusts the authority status of roles.\n\
        Any member with an authority role can use these higher commands.\n\n\
        Authority commands: `authorities`, `matchlive`, `prune`, `roleassign`, \
        `togglesongs`, `track`, `trackstream`.";

    MyCommand::new("authorities", "Adjust authority roles for a server")
        .help(help)
        .options(vec![add, list, remove])
        .authority()
}
