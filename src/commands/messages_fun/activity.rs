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
    let name = {
        let guild_id = msg.guild_id.unwrap();
        let guild_lock = guild_id.to_guild_cached(&ctx.cache).unwrap();
        if let Some(channel) = args.channel {
            if guild_lock.read().channels.contains_key(&channel) {
                channel.name(&ctx.cache).unwrap()
            } else {
                msg.channel_id.say(
                    &ctx.http,
                    "Could not find the specified channel in this server",
                )?;
                return Ok(());
            }
        } else {
            guild_lock.read().name.clone()
        }
    };
    let activity = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        mysql.activity_amount(args.channel.map(|channel| channel.0))?
    };
    let data = BasicEmbedData::create_activity(activity, name, args.channel.is_some());
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))?;
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}

pub struct MessageActivity {
    pub hour: usize,
    pub day: usize,
    pub week: usize,
    pub month: usize,
}

impl MessageActivity {
    pub fn new(hour: usize, day: usize, week: usize, month: usize) -> Self {
        Self {
            hour,
            day,
            week,
            month,
        }
    }
}
