use crate::{
    embeds::{EmbedData, RoleAssignEmbed},
    util::{constants::GENERAL_ISSUE, matcher, ApplicationCommandExt, MessageExt},
    Args, BotResult, CommandData, Context, Error,
};

use std::sync::Arc;
use twilight_model::{
    application::{
        command::{BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption},
        interaction::{application_command::CommandDataOption, ApplicationCommand},
    },
    id::{ChannelId, MessageId, RoleId},
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
        CommandData::Interaction { command } => slash_roleassign(ctx, command).await,
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

    if ctx.cache.role(role_id).is_none() {
        return data.error(&ctx, "Role not found in this guild").await;
    }

    // TODO: Check if bot has sufficient permissions to assign the role

    if ctx.cache.guild_channel(channel_id).is_none() {
        return data.error(&ctx, "Channel not found in this guild").await;
    }

    let msg = match ctx.http.message(channel_id, msg_id).exec().await {
        Ok(msg_res) => msg_res.model().await?,
        Err(why) => {
            let _ = data.error(&ctx, "No message found with this id").await;

            unwind_error!(
                warn,
                why,
                "(Channel,Message) ({},{}) for roleassign was not found: {}",
                channel_id,
                msg_id
            );

            return Ok(());
        }
    };

    let add_fut = ctx
        .psql()
        .add_role_assign(channel_id.0, msg_id.0, role_id.0);

    match add_fut.await {
        Ok(_) => debug!("Inserted into role_assign table"),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    }

    ctx.add_role_assign(channel_id, msg_id, role_id);
    let guild_id = data.guild_id().unwrap();
    let embed_data = RoleAssignEmbed::new(&ctx, msg, guild_id, role_id).await;
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
    fn args(args: &mut Args) -> Result<Self, &'static str> {
        let channel = match args.next() {
            Some(arg) => match matcher::get_mention_channel(arg) {
                Some(id) => ChannelId(id),
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
                Some(id) => RoleId(id),
                None => return Err("The third argument must be a role id."),
            },
            None => return Err("You must provide a channel, a message, and a role."),
        };

        Ok(Self { channel, msg, role })
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<Result<Self, &'static str>> {
        let mut channel = None;
        let mut msg = None;
        let mut role = None;

        let options = command.yoink_options();

        // TODO
        println!("{:#?}", options);

        for option in options {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "channel" => match value.parse() {
                        Ok(num) => channel = Some(ChannelId(num)),
                        Err(_) => bail_cmd_option!("roleassign channel", string, value),
                    },
                    "message" => match value.parse() {
                        Ok(num) => msg = Some(MessageId(num)),
                        Err(_) => {
                            let content =
                                "Could not parse message id. Be sure its a valid integer.";

                            return Ok(Err(content));
                        }
                    },
                    "role" => match value.parse() {
                        Ok(num) => role = Some(RoleId(num)),
                        Err(_) => bail_cmd_option!("roleassign role", string, value),
                    },
                    _ => bail_cmd_option!("roleassign", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("roleassign", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("roleassign", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("roleassign", subcommand, name)
                }
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

pub fn slash_roleassign_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "roleassign".to_owned(),
        default_permission: None,
        description: "Managing roles with reactions".to_owned(),
        id: None,
        options: vec![
            CommandOption::Channel(BaseCommandOptionData {
                description: "Specify the channel that contains the message".to_owned(),
                name: "channel".to_owned(),
                required: true,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify a message id".to_owned(),
                name: "message".to_owned(),
                required: true,
            }),
            CommandOption::Role(BaseCommandOptionData {
                description: "Specify a role that should be assigned".to_owned(),
                name: "role".to_owned(),
                required: true,
            }),
        ],
    }
}
