use std::sync::Arc;

use eyre::Report;
use twilight_model::{
    application::interaction::{application_command::CommandOptionValue, ApplicationCommand},
    id::{ChannelId, MessageId, RoleId},
};

use crate::{
    commands::{MyCommand, MyCommandOption},
    embeds::{EmbedData, RoleAssignEmbed},
    util::{constants::GENERAL_ISSUE, matcher, MessageExt},
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
        channel: channel_id,
        msg: msg_id,
        role: role_id,
    } = args;

    if !matches!(ctx.cache.contains(role_id).await, Ok(true)) {
        return data.error(&ctx, "Role not found in this guild").await;
    }

    if !matches!(ctx.cache.contains(channel_id).await, Ok(true)) {
        return data.error(&ctx, "Channel not found in this guild").await;
    }

    let msg = match ctx.http.message(channel_id, msg_id).exec().await {
        Ok(msg_res) => msg_res.model().await?,
        Err(why) => {
            let _ = data.error(&ctx, "No message found with this id").await;

            let wrap = format!(
                "(Channel,Message) ({},{}) for roleassign was not found",
                channel_id, msg_id
            );

            let report = Report::new(why).wrap_err(wrap);
            warn!("{:?}", report);

            return Ok(());
        }
    };

    let add_fut = ctx
        .psql()
        .add_role_assign(channel_id.get(), msg_id.get(), role_id.get());

    match add_fut.await {
        Ok(_) => debug!("Inserted into role_assign table"),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    }

    ctx.add_role_assign(channel_id, msg_id, role_id);
    let guild_id = data.guild_id().unwrap();
    let embed_data = RoleAssignEmbed::new(msg, guild_id, role_id).await;
    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}

struct RoleAssignArgs {
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

        Ok(Self { channel, msg, role })
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<Result<Self, &'static str>> {
        let mut channel = None;
        let mut role = None;
        let mut msg = None;

        for option in &command.data.options {
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

        let args = Self {
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

    let help = "With this command you can link a message to a role.\n\
        Whenever anyone reacts with any reaction to that message, they will gain that role.\n\
        If they remove a reaction from the message, they will lose the role.\n\
        __**Note**__: Roles can only be assigned if they are lower than some role of the assigner i.e. the bot.";

    MyCommand::new("roleassign", "Managing roles with reactions")
        .help(help)
        .options(vec![channel, message, role])
}
