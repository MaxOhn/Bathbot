use crate::{commands::checks::*, util::discord, MySQL};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::*,
    prelude::*,
};

#[command]
#[only_in("guild")]
#[checks(Authority)]
#[description = "If message tracking was enabled on the server, \
this will disable it **and remove all memoized messages of the server**.\n\
Since reversing this effect is rather expensive, you have to give a simple `yes` as argument.\n\
This command will disable commands such as `impersonate`, `hivemind`, ..."]
pub fn disabletracking(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let yes = args
        .single::<String>()
        .map(|arg| arg.to_lowercase().as_str() == "yes");
    if let Ok(true) = yes {
        // TODO
    } else {
        let response = msg.channel_id.say(
            &ctx.http,
            "To disable message tracking on this server you must provide \
            `yes` as argument, i.e. `<disabletracking yes`, to indicate \
            you are sure you want do delete my message memory of this server.",
        )?;
        discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    }
    Ok(())
}
