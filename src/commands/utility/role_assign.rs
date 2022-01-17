use std::{sync::Arc, time::Duration};

use eyre::Report;
use tokio::time::timeout;
use twilight_model::{
    application::interaction::{application_command::CommandOptionValue, ApplicationCommand},
    id::{ChannelId, MessageId, RoleId},
};

use crate::{
    commands::{MyCommand, MyCommandOption},
    util::{constants::GENERAL_ISSUE, matcher, MessageBuilder, MessageExt},
    Args, BotResult, CommandData, Context, Error,
};

#[command]
#[only_guilds()]
#[authority()]
#[short_desc("Managing roles with reactions")]
#[long_desc(
    "Assign a message to a role such that \
     when anyone reacts to that message, the member will \
     gain that role and and if they remove a reaction, \
     they lose the role.\n\
     The first argument must be the channel that contains the message, \
     the second must be the message id, and the third must be the role.\n\
     **Note:** Be sure the bot has sufficient privileges to assign the role."
)]
#[usage("[channel mention / channel id] [message id] [role mention / role id]")]
#[example("#general 681871156168753193 @Meetup")]
async fn roleassign(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => match RoleAssignArgs::args(&mut args) {
            Ok(roleassign_args) => {
                let data = CommandData::Message { msg, args, num };

                _roleassign(ctx, data, roleassign_args).await
            }
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => slash_roleassign(ctx, *command).await,
    }
}

async fn _roleassign(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    args: RoleAssignArgs,
) -> BotResult<()> {
    let RoleAssignArgs {
        kind,
        channel: channel_id,
        msg: msg_id,
        role: role_id,
    } = args;

    let role_pos = match ctx.cache.role(role_id, |role| role.position) {
        Ok(pos) => pos,
        Err(_) => return data.error(&ctx, "Role not found in this guild").await,
    };

    if ctx.cache.channel(channel_id, |_| ()).is_err() {
        return data.error(&ctx, "Channel not found in this guild").await;
    }

    let msg = match ctx.http.message(channel_id, msg_id).exec().await {
        Ok(msg_res) => msg_res.model().await?,
        Err(why) => {
            let _ = data.error(&ctx, "No message found with this id").await;

            let wrap = format!(
                "(Channel,Message) ({channel_id},{msg_id}) for roleassign was not found"
            );

            let report = Report::new(why).wrap_err(wrap);
            warn!("{:?}", report);

            return Ok(());
        }
    };

    let guild_id = data.guild_id().unwrap();

    match kind {
        Kind::Add => {
            let user = match ctx.cache.current_user() {
                Ok(user) => user,
                Err(_) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    bail!("CurrentUser not in cache");
                }
            };

            let has_permission_fut = async {
                ctx.cache.member(guild_id, user.id, |m| {
                    m.roles()
                        .iter()
                        .any(|&r| match ctx.cache.role(r, |r| r.position > role_pos) {
                            Ok(b) => b,
                            Err(_) => {
                                warn!("CurrentUser role {r} not in cache");

                                false
                            }
                        })
                })
            };

            match timeout(Duration::from_secs(5), has_permission_fut).await {
                Ok(Ok(true)) => {}
                Ok(Ok(false)) => {
                    let description = format!(
                        "To assign a role, one must have a role that is \
                        higher than the role to assign.\n\
                        The role <@&{role_id}> is higher than all my roles so I can't assign it."
                    );

                    return data.error(&ctx, description).await;
                }
                Ok(Err(_)) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    bail!(
                        "no member data in guild {guild_id} for CurrentUser in cache"
                    );
                }
                Err(_) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    bail!("timed out while checking role permissions");
                }
            }

            let add_fut = ctx
                .psql()
                .add_role_assign(channel_id.get(), msg_id.get(), role_id.get());

            match add_fut.await {
                Ok(_) => debug!("Inserted into role_assign table"),
                Err(err) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            }

            ctx.add_role_assign(channel_id, msg_id, role_id);

            let description = format!(
                "Whoever reacts to <@{author}>'s [message]\
                (https://discordapp.com/channels/{guild}/{channel}/{msg})\n\
                ```\n{content}\n```\n\
                in <#{channel_mention}> will be assigned the <@&{role_mention}> role!",
                author = msg.author.id,
                guild = guild_id,
                channel = msg.channel_id,
                msg = msg.id,
                content = msg.content,
                channel_mention = msg.channel_id,
                role_mention = role_id,
            );

            let builder = MessageBuilder::new().embed(description);
            data.create_message(&ctx, builder).await?;
        }
        Kind::Remove => {
            let remove_fut =
                ctx.psql()
                    .remove_role_assign(channel_id.get(), msg_id.get(), role_id.get());

            match remove_fut.await {
                Ok(true) => {
                    let description = format!(
                        "Reactions for <@{author}>'s [message]\
                        (https://discordapp.com/channels/{guild}/{channel}/{msg}) \
                        in <#{channel_mention}> will no longer assign the <@&{role_mention}> role.",
                        author = msg.author.id,
                        guild = guild_id,
                        channel = msg.channel_id,
                        msg = msg.id,
                        channel_mention = msg.channel_id,
                        role_mention = role_id,
                    );

                    debug!("Removed from role_assign table");
                    ctx.remove_role_assign(channel_id, msg_id, role_id);
                    let builder = MessageBuilder::new().embed(description);
                    data.create_message(&ctx, builder).await?;
                }
                Ok(false) => {
                    let description = format!(
                        "<@{author}>'s [message]\
                        (https://discordapp.com/channels/{guild}/{channel}/{msg}) \
                        in <#{channel_mention}> was not linked to the <@&{role_mention}> role to begin with.",
                        author = msg.author.id,
                        guild = guild_id,
                        channel = msg.channel_id,
                        msg = msg.id,
                        channel_mention = msg.channel_id,
                        role_mention = role_id,
                    );

                    data.error(&ctx, description).await?;
                }
                Err(err) => {
                    let _ = data.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            }
        }
    }

    Ok(())
}

enum Kind {
    Add,
    Remove,
}

struct RoleAssignArgs {
    kind: Kind,
    channel: ChannelId,
    msg: MessageId,
    role: RoleId,
}

impl RoleAssignArgs {
    fn args(args: &mut Args<'_>) -> Result<Self, &'static str> {
        let channel = match args.next() {
            Some(arg) => match matcher::get_mention_channel(arg) {
                Some(channel_id) => channel_id,
                None => return Err("The first argument must be a channel id."),
            },
            None => return Err("You must provide a channel, a message, and a role."),
        };

        let msg = match args.next() {
            Some(arg) => match arg.parse() {
                Ok(id) => MessageId(id),
                Err(_) => return Err("The second argument must be a message id."),
            },
            None => return Err("You must provide a channel, a message, and a role."),
        };

        let role = match args.next() {
            Some(arg) => match matcher::get_mention_role(arg) {
                Some(role_id) => role_id,
                None => return Err("The third argument must be a role id."),
            },
            None => return Err("You must provide a channel, a message, and a role."),
        };

        Ok(Self {
            kind: Kind::Add,
            channel,
            msg,
            role,
        })
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<Result<Self, &'static str>> {
        let mut channel = None;
        let mut role = None;
        let mut msg = None;

        let option = command
            .data
            .options
            .first()
            .ok_or(Error::InvalidCommandOptions)?;

        let kind = match &option.value {
            CommandOptionValue::SubCommand(options) => {
                for option in options {
                    match &option.value {
                        CommandOptionValue::String(value) => match option.name.as_str() {
                            "message" => match value.parse() {
                                Ok(0) => {
                                    let content = "Message id can't be `0`.";

                                    return Ok(Err(content));
                                }
                                Ok(num) => msg = Some(MessageId::new(num).unwrap()),
                                Err(_) => {
                                    let content =
                                        "Failed to parse message id. Be sure its a valid integer.";

                                    return Ok(Err(content));
                                }
                            },
                            _ => return Err(Error::InvalidCommandOptions),
                        },
                        CommandOptionValue::Channel(value) => channel = Some(*value),
                        CommandOptionValue::Role(value) => role = Some(*value),
                        _ => return Err(Error::InvalidCommandOptions),
                    }
                }

                match option.name.as_str() {
                    "add" => Kind::Add,
                    "remove" => Kind::Remove,
                    _ => return Err(Error::InvalidCommandOptions),
                }
            }
            _ => return Err(Error::InvalidCommandOptions),
        };

        let args = Self {
            kind,
            channel: channel.ok_or(Error::InvalidCommandOptions)?,
            msg: msg.ok_or(Error::InvalidCommandOptions)?,
            role: role.ok_or(Error::InvalidCommandOptions)?,
        };

        Ok(Ok(args))
    }
}

pub async fn slash_roleassign(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    match RoleAssignArgs::slash(&mut command)? {
        Ok(args) => _roleassign(ctx, command.into(), args).await,
        Err(content) => command.error(&ctx, content).await,
    }
}

pub fn define_roleassign() -> MyCommand {
    let channel =
        MyCommandOption::builder("channel", "Specify the channel that contains the message")
            .channel(true);

    let message_help = "Specify the message by providing its ID.\n\
        You can find the ID by rightclicking the message and clicking on `Copy ID`.\n\
        To see the `Copy ID` option, you must have `Settings > Advanced > Developer Mode` enabled.";

    let message = MyCommandOption::builder("message", "Specify a message id")
        .help(message_help)
        .string(Vec::new(), true);

    let role =
        MyCommandOption::builder("role", "Specify a role that should be assigned").role(true);

    let add_help = "Add role-assigning upon reaction on a message \
        i.e. make me add or remove a member's role when they (un)react to a message.";

    let add = MyCommandOption::builder("add", "Add role-assigning upon reaction on a message")
        .help(add_help)
        .subcommand(vec![channel, message, role]);

    let channel =
        MyCommandOption::builder("channel", "Specify the channel that contains the message")
            .channel(true);

    let message_help = "Specify the message by providing its ID.\n\
            You can find the ID by rightclicking the message and clicking on `Copy ID`.\n\
            To see the `Copy ID` option, you must have `Settings > Advanced > Developer Mode` enabled.";

    let message = MyCommandOption::builder("message", "Specify a message id")
        .help(message_help)
        .string(Vec::new(), true);

    let role = MyCommandOption::builder("role", "Specify a role that was assigned for the message")
        .role(true);

    let remove_help = "Remove role-assigning upon reaction on a message \
        i.e. I will no longer add or remove a member's role when they (un)react to a message.";

    let remove =
        MyCommandOption::builder("remove", "Remove role-assigning upon reaction on a message")
            .help(remove_help)
            .subcommand(vec![channel, message, role]);

    let help = "With this command you can link a message to a role.\n\
        Whenever anyone reacts with __any__ reaction to that message, they will gain that role.\n\
        If they remove a reaction from the message, they will lose the role.\n\
        __**Note**__: Roles can only be assigned if they are lower than some role of the assigner i.e. the bot.";

    MyCommand::new("roleassign", "Managing roles with reactions")
        .help(help)
        .options(vec![add, remove])
}
