use crate::{commands::checks::*, util::discord, Guilds, MySQL};
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
pub async fn disabletracking(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    {
        let data = ctx.data.read().await;
        let guilds = data.get::<Guilds>().expect("Could not get Guilds");
        if !guilds.get(&msg.guild_id.unwrap()).unwrap().message_tracking {
            msg.channel_id
                .say(
                    &ctx.http,
                    "Message tracking is already disabled for this server.",
                )
                .await?;
            return Ok(());
        }
    }
    let yes = args
        .single::<String>()
        .map(|arg| arg.to_lowercase().as_str() == "yes");
    if let Ok(true) = yes {
        let guild_id = msg.guild_id.unwrap();
        {
            let mut data = ctx.data.write().await;
            let guilds = data.get_mut::<Guilds>().expect("Could not get Guilds");
            guilds
                .get_mut(&guild_id)
                .unwrap_or_else(|| panic!("Guild {} not found", guild_id.0))
                .message_tracking = false;
        }
        let channels: Vec<_> = guild_id
            .to_guild_cached(&ctx.cache)
            .await
            .expect("Guild not found")
            .read()
            .await
            .channels(&ctx.http)
            .await?
            .into_iter()
            .map(|(id, _)| id.0)
            .collect();
        {
            let data = ctx.data.read().await;
            let mysql = data.get::<MySQL>().expect("Could not get MySQL");
            if let Err(why) = mysql.update_guild_tracking(guild_id.0, false) {
                warn!("Error while updating message_tracking: {}", why);
            }
            if let Err(why) = mysql.remove_channel_msgs(&channels) {
                warn!("Error while removing messages from channels: {}", why);
            }
        }
    } else {
        let response = msg
            .channel_id
            .say(
                &ctx.http,
                "To disable message tracking on this server you must provide \
            `yes` as argument,\ni.e. `<disabletracking yes`, to indicate \
            you are sure you want do delete my message memory of this server.",
            )
            .await?;
        discord::reaction_deletion(&ctx, response, msg.author.id);
    }
    Ok(())
}
