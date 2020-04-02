use crate::{arguments::MarkovChannelArgs, embeds::BasicEmbedData, util::discord, Guilds, MySQL};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::*,
    prelude::*,
};

#[command]
#[only_in("guild")]
#[description = "Display how active the server or channel has been in the last hour / day / week / month"]
pub fn activity(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    {
        let data = ctx.data.read();
        let guilds = data.get::<Guilds>().expect("Could not get Guilds");
        if !guilds.get(&msg.guild_id.unwrap()).unwrap().message_tracking {
            msg.channel_id.say(
                &ctx.http,
                "No messages tracked on this guild yet. \
                To enable message tracking, use the `<enabletracking` command first.",
            )?;
            return Ok(());
        }
    }
    let args = MarkovChannelArgs::new(args);
    let channel = args.channel;
    let mut strings = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.impersonate_strings(None, args.channel)?
    };
    let stats = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.message_stats(&channels, msg.channel_id.0, user.id.0)?
    };
    let data = BasicEmbedData::create_messagestats(stats, &user.name);
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))?;
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}

pub struct MessageActivity {
    hour: u32,
    day: u32,
    week: u32,
    month: u32,
}
