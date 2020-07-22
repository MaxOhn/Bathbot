use super::super::command_issue;
use crate::{
    arguments::{Args, RoleAssignArgs},
    bail,
    embeds::{EmbedData, RoleAssignEmbed},
    util::MessageExt,
    BotResult, Context,
};

use std::sync::Arc;
use twilight::builders::embed::EmbedBuilder;
use twilight::model::{
    channel::Message,
    id::{ChannelId, MessageId, RoleId},
};

#[command]
// #[only_in("guild")]
// #[checks(Authority)]
#[short_desc("Mangaging roles with reactions")]
#[long_desc(
    "Assign a message to a role such that \
     when anyone reacts to that message, the member will \
     gain that role and and if they remove a reaction, \
     they lose the role\n\
     The first argument must be the channel that contains the message, \
     the second must be the message id, and the third must be the role."
)]
#[usage("[channel mention / channel id] [message id] [role mention / role id]")]
#[example("#general 681871156168753193 @Meetup")]
async fn roleassign(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let args = Args::new(msg.content.clone());
    let args = match RoleAssignArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.respond(&ctx, err_msg).await?;
            return Ok(());
        }
    };
    let channel = args.channel_id;
    let msg_id = args.message_id;
    let role = args.role_id;
    let psql = &ctx.clients.psql;
    match psql.add_role_assign(channel.0, msg_id.0, role.0).await {
        Ok(_) => debug!("Inserted into role_assign table"),
        Err(why) => {
            msg.respond(&ctx, command_issue("roleassign")).await?;
            return Err(why.into());
        }
    }
    ctx.role_assigns.insert((channel.0, msg_id.0), role.0);
    let message = match ctx.http.message(channel, msg_id).await? {
        Some(message) => message,
        None => {
            bail!(
                "Message of role_assign (({},{}),{}) was not found",
                channel,
                msg_id,
                role
            );
        }
    };
    let data = RoleAssignEmbed::new(&ctx, message, msg.guild_id.unwrap(), role).await;
    let eb = data.build(EmbedBuilder::new());
    msg.build_response(&ctx, |m| m.embed(eb.build())).await?;
    Ok(())
}
