use std::{borrow::Cow, sync::Arc, time::Duration};

use command_macros::{command, SlashCommand};
use eyre::Report;
use tokio::time::timeout;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    channel::Message,
    id::{
        marker::{ChannelMarker, MessageMarker, RoleMarker},
        Id,
    },
};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    util::{builder::MessageBuilder, constants::GENERAL_ISSUE, matcher, ApplicationCommandExt},
    BotResult, Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "roleassign",
    help = "With this command you can link a message to a role.\n\
    Whenever anyone reacts with __any__ reaction to that message, they will gain that role.\n\
    If they remove a reaction from the message, they will lose the role.\n\
    __**Note**__: Roles can only be assigned if they are lower than some role of the assigner i.e. the bot."
)]
#[flags(AUTHORITY, ONLY_GUILDS)]
/// Manage roles with reactions
pub enum RoleAssign<'a> {
    #[command(name = "add")]
    Add(RoleAssignAdd<'a>),
    #[command(name = "remove")]
    Remove(RoleAssignRemove<'a>),
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "add",
    help = "Add role-assigning upon reaction on a message \
    i.e. make me add or remove a member's role when they (un)react to a message."
)]
/// Add role-assigning upon reaction on a message
pub struct RoleAssignAdd<'a> {
    /// Specify the channel that contains the message
    channel: Id<ChannelMarker>,
    #[command(help = "Specify the message by providing its ID.\n\
    You can find the ID by rightclicking the message and clicking on `Copy ID`.\n\
    To see the `Copy ID` option, you must have `Settings > Advanced > Developer Mode` enabled.")]
    /// Specify a message id
    message: Cow<'a, str>,
    /// Specify a role that should be assigned
    role: Id<RoleMarker>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "remove",
    help = "Remove role-assigning upon reaction on a message \
    i.e. I will no longer add or remove a member's role when they (un)react to a message."
)]
/// Remove role-assigning upon reaction on a message
pub struct RoleAssignRemove<'a> {
    /// Specify the channel that contains the message
    channel: Id<ChannelMarker>,
    #[command(help = "Specify the message by providing its ID.\n\
    You can find the ID by rightclicking the message and clicking on `Copy ID`.\n\
    To see the `Copy ID` option, you must have `Settings > Advanced > Developer Mode` enabled.")]
    /// Specify a message id
    message: Cow<'a, str>,
    /// Specify a role that was assigned for the message
    role: Id<RoleMarker>,
}

impl<'m> RoleAssign<'m> {
    fn args(args: &mut Args<'m>) -> Result<Self, &'static str> {
        let channel = match args.next() {
            Some(arg) => match matcher::get_mention_channel(arg) {
                Some(channel_id) => channel_id,
                None => return Err("The first argument must be a channel id."),
            },
            None => return Err("You must provide a channel, a message, and a role."),
        };

        let msg = match args.next() {
            Some(arg) => Cow::Borrowed(arg),
            None => return Err("You must provide a channel, a message, and a role."),
        };

        let role = match args.next() {
            Some(arg) => match matcher::get_mention_role(arg) {
                Some(role_id) => role_id,
                None => return Err("The third argument must be a role id."),
            },
            None => return Err("You must provide a channel, a message, and a role."),
        };

        Ok(Self::Add(RoleAssignAdd { channel, msg, role }))
    }
}

#[command]
#[desc("Managing roles with reactions")]
#[help(
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
#[flags(AUTHORITY, ONLY_GUILDS)]
#[group(Utility)]
async fn prefix_roleassign(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> BotResult<()> {
    match RoleAssign::args(&mut args) {
        Ok(args) => roleassign(ctx, msg.into(), args).await,
        Err(content) => msg.error(&ctx, content).await,
    }
}

pub async fn slash_roleassign(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let args = RoleAssign::from_iteraction(command.input_data())?;

    roleassign(ctx, command.into(), args).await
}

async fn roleassign(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RoleAssign<'_>,
) -> BotResult<()> {
    match args {
        RoleAssign::Add(add) => {
            let msg = match parse_msg_id(add.message) {
                Ok(id) => id,
                Err(content) => return orig.error(&ctx, content).await,
            };

            if ctx.cache.channel(add.channel_id, |_| ()).is_err() {
                return orig.error(&ctx, "Channel not found in this guild").await;
            }

            let (role_pos, msg) = match retrieve_data(&ctx, add.channel, msg, add.role).await? {
                Ok(tuple) => tuple,
                Err(content) => return orig.error(&ctx, content).await,
            };

            let guild = orig.guild_id().unwrap();

            let user = match ctx.cache.current_user() {
                Ok(user) => user,
                Err(_) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    bail!("CurrentUser not in cache");
                }
            };

            let has_permission_fut = async {
                ctx.cache.member(guild, user.id, |m| {
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
                        The role <@&{role}> is higher than all my roles so I can't assign it.",
                        role = add.role,
                    );

                    return orig.error(&ctx, description).await;
                }
                Ok(Err(_)) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    bail!("no member data in guild {guild} for CurrentUser in cache");
                }
                Err(_) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    bail!("timed out while checking role permissions");
                }
            }

            let add_fut =
                ctx.psql()
                    .add_role_assign(add.channel.get(), msg.id.get(), add.role.get());

            match add_fut.await {
                Ok(_) => debug!("Inserted into role_assign table"),
                Err(err) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            }

            ctx.add_role_assign(add.channel, msg.id, add.role);

            let description = format!(
                "Whoever reacts to <@{author}>'s [message]\
                (https://discordapp.com/channels/{guild}/{channel}/{msg})\n\
                ```\n{content}\n```\n\
                in <#{channel_mention}> will be assigned the <@&{role_mention}> role!",
                author = msg.author.id,
                channel = msg.channel_id,
                msg = msg.id,
                content = msg.content,
                channel_mention = msg.channel_id,
                role_mention = add.role,
            );

            let builder = MessageBuilder::new().embed(description);
            orig.create_message(&ctx, &builder).await?;
        }
        RoleAssign::Remove(remove) => {
            let msg = match parse_msg_id(remove.message) {
                Ok(id) => id,
                Err(content) => return orig.error(&ctx, content).await,
            };

            if ctx.cache.channel(remove.channel_id, |_| ()).is_err() {
                return orig.error(&ctx, "Channel not found in this guild").await;
            }

            let (role_pos, msg) =
                match retrieve_data(&ctx, remove.channel, msg, remove.role).await? {
                    Ok(tuple) => tuple,
                    Err(content) => return orig.error(&ctx, content).await,
                };

            let guild = orig.guild_id().unwrap();

            let remove_fut = ctx.psql().remove_role_assign(
                remove.channel.get(),
                msg.id.get(),
                remove.role.get(),
            );

            match remove_fut.await {
                Ok(true) => {
                    let description = format!(
                        "Reactions for <@{author}>'s [message]\
                        (https://discordapp.com/channels/{guild}/{channel}/{msg}) \
                        in <#{channel_mention}> will no longer assign the <@&{role_mention}> role.",
                        author = msg.author.id,
                        channel = msg.channel_id,
                        msg = msg.id,
                        channel_mention = msg.channel_id,
                        role_mention = remove.role,
                    );

                    debug!("Removed from role_assign table");
                    ctx.remove_role_assign(remove.channel, msg.id, remove.role);
                    let builder = MessageBuilder::new().embed(description);
                    orig.create_message(&ctx, &builder).await?;
                }
                Ok(false) => {
                    let description = format!(
                        "<@{author}>'s [message]\
                        (https://discordapp.com/channels/{guild}/{channel}/{msg}) \
                        in <#{channel_mention}> was not linked to the <@&{role_mention}> role to begin with.",
                        author = msg.author.id,
                        channel = msg.channel_id,
                        msg = msg.id,
                        channel_mention = msg.channel_id,
                        role_mention = remove.role,
                    );

                    orig.error(&ctx, description).await?;
                }
                Err(err) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            }
        }
    }

    Ok(())
}

fn parse_msg_id(msg: Cow<'_, str>) -> Result<Id<MessageMarker>, &'static str> {
    match msg.parse() {
        Ok(id) => Ok(Id::new(id)),
        Err(_) => Err("Failed to parse message id. Be sure its a valid integer."),
    }
}

async fn retrieve_data(
    ctx: &Context,
    channel: Id<ChannelMarker>,
    msg: Id<MessageMarker>,
    role: Id<RoleMarker>,
) -> BotResult<Result<(i64, Message), &'static str>> {
    let role_pos = match ctx.cache.role(role, |role| role.position) {
        Ok(pos) => pos,
        Err(_) => return Ok(Err("Role not found in this guild")),
    };

    let msg = match ctx.http.message(channel, msg).exec().await {
        Ok(msg_res) => msg_res.model().await?,
        Err(err) => {
            let wrap = format!("(Channel,Message) ({channel},{msg}) for roleassign was not found");
            let report = Report::new(err).wrap_err(wrap);
            warn!("{:?}", report);

            return Ok(Err("No message found with this id"));
        }
    };

    Ok(Ok((role_pos, msg)))
}
