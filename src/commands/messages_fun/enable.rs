use crate::{commands::checks::*, database::InsertableMessage, util::discord, MySQL};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::{channel::GuildChannel, prelude::*},
    prelude::*,
};

#[command]
#[only_in("guild")]
#[checks(Authority)]
#[description = "This command only needs to be used once in a server.\n\
If message tracking gets enabled, I will download all messages of this server, \
memorize them and also add all new future messages in the database.\n\
Since processing this command might take a very long time (maybe hours), \
you have to give a simple `yes` as argument.\n\
This command will enable commands such as `impersonate`, `hivemind`, ..."]
pub fn enabletracking(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let yes = args
        .single::<String>()
        .map(|arg| arg.to_lowercase().as_str() == "yes");
    if let Ok(true) = yes {
        // TODO
    } else {
        let response = msg.channel_id.say(
            &ctx.http,
            "To enable message tracking on this server you must provide \
            `yes` as argument, i.e. `<enabletracking yes`, to indicate \
            you are sure you want start downloading all messages of this server \
            which might take a long time (maybe hours).",
        )?;
        discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    }
    Ok(())
}

// Download all messages inside the guild
fn download_all_messages(ctx: &Context, guild: &Guild) {
    let channels = match guild.channels(&ctx.http) {
        Ok(channels) => channels,
        Err(why) => {
            warn!("Could not get channels of server: {}", why);
            return;
        }
    };
    let channels: Vec<GuildChannel> = channels
        .into_iter()
        .filter(|(_, guild_channel)| guild_channel.bitrate.is_none())
        .filter(|(_, guild_channel)| guild_channel.last_message_id.is_some())
        .map(|(_, guild_channel)| guild_channel)
        .collect();
    let data = ctx.data.read();
    let mysql = data.get::<MySQL>().expect("Could not get MySQL");
    for channel in channels {
        let mut channel_messages = Vec::new();
        let channel_id = channel.id;
        let biggest_id = channel.last_message_id.unwrap().0;
        match mysql.biggest_id_exists(biggest_id) {
            Ok(false) => {}
            Ok(true) => continue,
            Err(why) => {
                error!("Error getting biggest_id_exists: {}", why);
                continue;
            }
        }
        if let Some(last_id) = mysql.latest_id_for_channel(channel_id.0) {
            match channel_id.messages(&ctx.http, |g| g.after(MessageId(last_id)).limit(100)) {
                Ok(res) => channel_messages = res,
                Err(why) => warn!("Error getting messages: {}", why),
            }
        } else {
            match channel_id.messages(&ctx.http, |g| g.after(0).limit(100)) {
                Ok(res) => channel_messages = res,
                Err(why) => warn!("Error getting messages: {}", why),
            }
        }
        while !channel_messages.is_empty() {
            #[allow(clippy::unreadable_literal)]
            let transformed_message_vec: Vec<_> = channel_messages
                .iter()
                .filter(|msg| msg.author.id.0 != 460234151057031168) // yentis' bot spammer
                .filter(|msg| !msg.content.is_empty())
                .filter(|msg| !msg.content.starts_with('<'))
                .filter(|msg| !msg.content.starts_with('>'))
                .filter(|msg| !msg.content.starts_with('!'))
                .map(|msg| InsertableMessage {
                    id: msg.id.0,
                    channel_id: msg.channel_id.0,
                    author: msg.author.id.0,
                    content: msg.content.clone(),
                    timestamp: msg.timestamp.naive_utc(),
                })
                .collect();
            if transformed_message_vec.is_empty() {
                break;
            }
            info!(
                "Storing {} messages from #{} on {}",
                transformed_message_vec.len(),
                channel.name,
                guild.name
            );
            let _ = mysql.insert_msgs(&transformed_message_vec);
            if let Some(last_id) = mysql.latest_id_for_channel(channel_id.0) {
                if last_id >= biggest_id {
                    break;
                } else {
                    let r#try =
                        channel_id.messages(&ctx.http, |g| g.after(MessageId(last_id)).limit(100));
                    match r#try {
                        Err(why) => warn!("Error getting messages: {}", why),
                        _ => channel_messages = r#try.unwrap(),
                    }
                }
            } else {
                let r#try = channel_id.messages(&ctx.http, |g| g.after(0).limit(100));
                match r#try {
                    Err(why) => warn!("Error getting messages: {}", why),
                    _ => channel_messages = r#try.unwrap(),
                }
            }
        }
    }
    info!("Downloaded all messages for guild {}", guild.name);
}
