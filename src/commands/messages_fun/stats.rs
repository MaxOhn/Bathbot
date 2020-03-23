use crate::{arguments::DiscordUserArgs, embeds::BasicEmbedData, util::discord, MySQL};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::*,
    prelude::*,
};

#[command]
#[only_in("guild")]
#[description = "Display some stats about the message database"]
pub fn messagestats(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let user = match DiscordUserArgs::new(args, &ctx, msg.guild_id.unwrap()) {
        Ok(args) => args.user,
        Err(_) => msg.author.clone(),
    };
    let channels: Vec<_> = msg
        .guild_id
        .unwrap()
        .channels(&ctx.http)?
        .keys()
        .map(|id| id.0)
        .collect();
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

pub struct MessageStats {
    //pub table_size: usize,
    pub total_msgs: usize,
    pub guild_msgs: usize,
    pub channel_msgs: usize,
    pub author_msgs: usize,
    pub author_avg: f32,
    pub single_words: Vec<(String, usize)>,
}

impl MessageStats {
    pub fn new(
        //table_size: Option<usize>,
        total_msgs: usize,
        guild_msgs: usize,
        channel_msgs: usize,
        author_msgs: usize,
        author_avg: f32,
        single_words: Vec<(String, usize)>,
    ) -> Self {
        Self {
            //table_size,
            total_msgs,
            guild_msgs,
            channel_msgs,
            author_msgs,
            author_avg,
            single_words,
        }
    }
}
