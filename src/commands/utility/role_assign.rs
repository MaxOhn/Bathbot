use crate::{
    arguments::{Args, RoleAssignArgs},
    embeds::{EmbedData, RoleAssignEmbed},
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::Message;

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
async fn roleassign(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = match RoleAssignArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };

    let channel = args.channel_id;
    let msg_id = args.message_id;
    let role = args.role_id;

    if ctx.cache.role(role).is_none() {
        return msg.error(&ctx, "Role not found in this guild").await;
    }

    if ctx.cache.guild_channel(channel).is_none() {
        return msg.error(&ctx, "Channel not found in this guild").await;
    }

    let message = match ctx.http.message(channel, msg_id).exec().await {
        Ok(msg_res) => msg_res.model().await?,
        Err(why) => {
            let _ = msg.error(&ctx, "No message found with this id").await;

            unwind_error!(
                warn,
                why,
                "(Channel,Message) ({},{}) for roleassign was not found: {}",
                channel,
                msg_id
            );

            return Ok(());
        }
    };

    match ctx
        .psql()
        .add_role_assign(channel.0, msg_id.0, role.0)
        .await
    {
        Ok(_) => debug!("Inserted into role_assign table"),
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    }

    ctx.add_role_assign(channel, msg_id, role);
    let data = RoleAssignEmbed::new(&ctx, message, msg.guild_id.unwrap(), role).await;
    let embed = &[data.into_builder().build()];
    msg.build_response(&ctx, |m| m.embeds(embed)).await?;

    Ok(())
}
