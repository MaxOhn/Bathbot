use crate::{
    commands::{arguments::ArgParser, checks::*},
    database::MySQL,
    ReactionTracker,
};

use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::{
        id::{ChannelId, MessageId, RoleId},
        prelude::Message,
    },
    prelude::Context,
    utils::MessageBuilder,
};

#[command]
#[checks(Authority)]
#[description = "Assign a message and its channel to a role such that \
                 when anyone reacts to that message, the member will \
                 gain that role and and if they remove a reaction, \
                 they lose the role"]
fn roleassign(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let mut arg_parser = ArgParser::new(args);
    let channel = match arg_parser.get_next_channel() {
        Some(channel) => channel,
        None => {
            msg.channel_id.say(
                &ctx.http,
                "The first argument must be either a channel \
                 id or just a mention of a channel",
            )?;
            return Ok(());
        }
    };
    let message = match arg_parser.get_next_u64() {
        Some(id) => MessageId(id),
        None => {
            msg.channel_id
                .say(&ctx.http, "The second argument must be the id of a message")?;
            return Ok(());
        }
    };
    let role = match arg_parser.get_next_role() {
        Some(role) => role,
        None => {
            msg.channel_id.say(
                &ctx.http,
                "The third argument must be either a role \
                 id or just a mention of a role",
            )?;
            return Ok(());
        }
    };
    {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.add_role_assign(channel.0, message.0, role.0) {
            msg.channel_id.say(
                &ctx.http,
                "Some issue while inserting into database, blame bade",
            )?;
            return Err(CommandError::from(why.to_string()));
        }
    }
    {
        let mut data = ctx.data.write();
        let reaction_tracker = data
            .get_mut::<ReactionTracker>()
            .expect("Could not get ReactionTracker");
        reaction_tracker.insert((ChannelId(channel.0), MessageId(message.0)), RoleId(role.0));
    }
    let message = channel.message(&ctx.http, message)?;

    let content = MessageBuilder::new()
        .push_line("Whoever reacts on the message")
        .push_line("---")
        .push_line_safe(&message.content) // TODO: try as quoted once its fixed in serenity
        .push_line("---")
        .push("in ")
        .channel(channel)
        .push(" will be assigned the ")
        .role(role)
        .push(" role!")
        .build();
    msg.channel_id.say(&ctx.http, content)?;
    Ok(())
}
