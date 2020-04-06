use crate::{
    arguments::RoleAssignArgs, commands::checks::*, database::MySQL, util::discord, ReactionTracker,
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
#[only_in("guild")]
#[checks(Authority)]
#[description = "Assign a message and its channel to a role such that \
                 when anyone reacts to that message, the member will \
                 gain that role and and if they remove a reaction, \
                 they lose the role"]
#[usage = "[channel mention / channel id] [message id] [role mention / role id]"]
#[example = "#general 681871156168753193 @Meetup"]
async fn roleassign(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let args = match RoleAssignArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            let response = msg.channel_id.say(&ctx.http, err_msg).await?;
            discord::save_response_owner(response.id, msg.author.id, ctx.data.clone()).await;
            return Ok(());
        }
    };
    let channel = args.channel_id;
    let message = args.message_id;
    let role = args.role_id;
    {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.add_role_assign(channel.0, message.0, role.0) {
            msg.channel_id
                .say(
                    &ctx.http,
                    "Some issue while inserting into database, blame bade",
                )
                .await?;
            return Err(CommandError::from(why.to_string()));
        }
    }
    {
        let mut data = ctx.data.write().await;
        let reaction_tracker = data
            .get_mut::<ReactionTracker>()
            .expect("Could not get ReactionTracker");
        reaction_tracker.insert((ChannelId(channel.0), MessageId(message.0)), RoleId(role.0));
    }
    let message = channel.message(&ctx.http, message).await?;

    let content = MessageBuilder::new()
        .push_line("Whoever reacts on the message")
        .push_line("---")
        .push_line_safe(&message.content) // TODO: try as quoted once its fixed in serenity
        .push_line("---")
        .push("in ")
        .channel(channel)
        .await
        .push(" will be assigned the ")
        .role(role)
        .await
        .push(" role!")
        .build();
    let response = msg.channel_id.say(&ctx.http, content).await?;

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone()).await;
    Ok(())
}
