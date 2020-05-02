use crate::{
    arguments::RoleAssignArgs, commands::checks::*, database::MySQL, embeds::BasicEmbedData,
    util::discord, ReactionTracker,
};

use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::{
        channel::Message,
        id::{ChannelId, MessageId, RoleId},
    },
    prelude::Context,
};

#[command]
#[only_in("guild")]
#[checks(Authority)]
#[description = "Assign a message to a role such that \
                 when anyone reacts to that message, the member will \
                 gain that role and and if they remove a reaction, \
                 they lose the role\n\
                 The first argument must be the channel that contains the message, \
                 the second must be the message id, and the third must be the role."]
#[usage = "[channel mention / channel id] [message id] [role mention / role id]"]
#[example = "#general 681871156168753193 @Meetup"]
async fn roleassign(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let args = match RoleAssignArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            let response = msg.channel_id.say(&ctx.http, err_msg).await?;
            discord::reaction_deletion(&ctx, response, msg.author.id).await;
            return Ok(());
        }
    };
    let channel = args.channel_id;
    let message = args.message_id;
    let role = args.role_id;
    {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        match mysql.add_role_assign(channel.0, message.0, role.0) {
            Ok(_) => debug!("Inserted into role_assign table"),
            Err(why) => {
                msg.channel_id
                    .say(&ctx.http, "Some issue while inserting into DB, blame bade")
                    .await?;
                return Err(CommandError::from(why.to_string()));
            }
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
    let data =
        BasicEmbedData::create_roleassign(message, msg.guild_id.unwrap(), role, &ctx.cache).await;
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;
    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}
