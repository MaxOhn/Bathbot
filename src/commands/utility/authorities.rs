use crate::{
    bail,
    util::{
        constants::{GENERAL_ISSUE, OWNER_USER_ID},
        matcher, ApplicationCommandExt, MessageExt,
    },
    Args, BotResult, CommandData, Context, Error, MessageBuilder,
};

use std::{fmt::Write, sync::Arc};
use twilight_model::{
    application::{
        command::{BaseCommandOptionData, Command, CommandOption, OptionsCommandOptionData},
        interaction::{application_command::CommandDataOption, ApplicationCommand},
    },
    guild::{Permissions, Role},
    id::RoleId,
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
            match AuthorityCommandKind::args(&ctx, &mut args) {
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

            // Make sure the author is still an authority after applying new roles
            if !(ctx.cache.is_guild_owner(guild_id, author_id) || author_id.0 == OWNER_USER_ID) {
                match ctx.cache.member(guild_id, author_id) {
                    Some(member) => {
                        let still_authority = member
                            .roles
                            .iter()
                            .filter_map(|&role_id| ctx.cache.role(role_id))
                            .any(|role| {
                                role.permissions.contains(Permissions::ADMINISTRATOR)
                                    || roles.iter().any(|&new| new == role.id.0 && new != role_id)
                            });

                        if !still_authority {
                            let content = "You cannot set authority roles to something \
                                that would make you lose authority status.";

                            return data.error(&ctx, content).await;
                        }
                    }
                    None => {
                        let _ = data.error(&ctx, GENERAL_ISSUE).await;

                        bail!("member {} not cached for guild {}", author_id, guild_id);
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
            if !(ctx.cache.is_guild_owner(guild_id, author_id) || author_id.0 == OWNER_USER_ID) {
                match ctx.cache.member(guild_id, author_id) {
                    Some(member) => {
                        let still_authority = member
                            .roles
                            .iter()
                            .filter_map(|&role_id| ctx.cache.role(role_id))
                            .any(|role| {
                                role.permissions.contains(Permissions::ADMINISTRATOR)
                                    || roles.iter().any(|new| new.id == role.id)
                            });

                        if !still_authority {
                            let content = "You cannot set authority roles to something \
                                that would make you lose authority status.";

                            return data.error(&ctx, content).await;
                        }
                    }
                    None => {
                        let _ = data.error(&ctx, GENERAL_ISSUE).await;

                        bail!("member {} not cached for guild {}", author_id, guild_id);
                    }
                }
            }

            let update_fut = ctx.update_guild_config(guild_id, move |config| {
                config.authorities = roles.into_iter().map(|role| role.id.0).collect();
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
    Replace(Vec<Role>),
}

fn parse_role(ctx: &Context, arg: &str) -> Result<Role, String> {
    let role_id = match matcher::get_mention_role(arg) {
        Some(id) => RoleId(id),
        None => return Err(format!("Expected role mention or role id, got `{}`", arg)),
    };

    match ctx.cache.role(role_id) {
        Some(role) => Ok(role),
        None => Err(format!("No role with id {} found in this guild", role_id)),
    }
}

impl AuthorityCommandKind {
    fn args(ctx: &Context, args: &mut Args) -> Result<Self, String> {
        let mut roles = match args.next() {
            Some("-show") | Some("show") => return Ok(Self::List),
            Some(arg) => vec![parse_role(ctx, arg)?],
            None => return Ok(Self::Replace(Vec::new())),
        };

        for arg in args.take(9) {
            roles.push(parse_role(ctx, arg)?);
        }

        Ok(Self::Replace(roles))
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let mut kind = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, .. } => {
                    bail_cmd_option!("authorites", string, name)
                }
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("authorites", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("authorites", boolean, name)
                }
                CommandDataOption::SubCommand { name, options } => match name.as_str() {
                    "add" => {
                        let mut role = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "role" => match value.parse() {
                                        Ok(num) => role = Some(num),
                                        Err(_) => {
                                            bail_cmd_option!("authorities add role", string, value)
                                        }
                                    },
                                    _ => bail_cmd_option!("authorities add", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("authorities add", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("authorities add", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("authorities add", subcommand, name)
                                }
                            }
                        }

                        let role = role.ok_or(Error::InvalidCommandOptions)?;
                        kind = Some(AuthorityCommandKind::Add(role));
                    }
                    "list" => kind = Some(AuthorityCommandKind::List),
                    "remove" => {
                        let mut role = None;

                        for option in options {
                            match option {
                                CommandDataOption::String { name, value } => match name.as_str() {
                                    "role" => match value.parse() {
                                        Ok(num) => role = Some(num),
                                        Err(_) => {
                                            bail_cmd_option!(
                                                "authorities remove role",
                                                string,
                                                value
                                            )
                                        }
                                    },
                                    _ => bail_cmd_option!("authorities remove", string, name),
                                },
                                CommandDataOption::Integer { name, .. } => {
                                    bail_cmd_option!("authorities remove", integer, name)
                                }
                                CommandDataOption::Boolean { name, .. } => {
                                    bail_cmd_option!("authorities remove", boolean, name)
                                }
                                CommandDataOption::SubCommand { name, .. } => {
                                    bail_cmd_option!("authorities remove", subcommand, name)
                                }
                            }
                        }

                        let role = role.ok_or(Error::InvalidCommandOptions)?;
                        kind = Some(AuthorityCommandKind::Remove(role));
                    }
                    _ => bail_cmd_option!("authorites", subcommand, name),
                },
            }
        }

        kind.ok_or(Error::InvalidCommandOptions)
    }
}

pub async fn slash_authorities(
    ctx: Arc<Context>,
    mut command: ApplicationCommand,
) -> BotResult<()> {
    let args = AuthorityCommandKind::slash(&mut command)?;

    _authorities(ctx, command.into(), args).await
}

pub fn slash_authorities_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "authorities".to_owned(),
        default_permission: None,
        description: "Adjust authority roles for a server".to_owned(),
        id: None,
        options: vec![
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Add authority status to a role".to_owned(),
                name: "add".to_owned(),
                options: vec![CommandOption::Role(BaseCommandOptionData {
                    description: "Specify the role that should gain authority status".to_owned(),
                    name: "role".to_owned(),
                    required: true,
                })],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Display all current authority roles".to_owned(),
                name: "list".to_owned(),
                options: vec![],
                required: false,
            }),
            CommandOption::SubCommand(OptionsCommandOptionData {
                description: "Remove authority status for a role".to_owned(),
                name: "remove".to_owned(),
                options: vec![CommandOption::Role(BaseCommandOptionData {
                    description: "Specify the role that should lose authority status".to_owned(),
                    name: "role".to_owned(),
                    required: true,
                })],
                required: false,
            }),
        ],
    }
}
