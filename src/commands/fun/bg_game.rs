use crate::util::discord;

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[description = "Background game coming eventually:tm:"]
#[aliases("bg")]
fn backgroundgame(ctx: &mut Context, msg: &Message, _args: Args) -> CommandResult {
    let response = msg.channel_id.say(
        &ctx.http,
        "The background game is not yet implemented on \
         this bot version. Will be done eventually:tm:",
    )?;

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}
