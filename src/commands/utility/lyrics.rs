use crate::{commands::checks::*, database::MySQL, util::MessageExt, Guilds};

use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::collections::hash_map::Entry;

#[command]
#[only_in("guild")]
#[checks(Authority)]
#[description = "Toggle whether song commands can be used in this server. \
Defaults to `true`"]
async fn lyrics(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    let new_bool = {
        let mut data = ctx.data.write().await;
        let guilds = data.get_mut::<Guilds>().unwrap();
        match guilds.entry(guild_id) {
            Entry::Occupied(mut entry) => {
                let new_bool = !entry.get().with_lyrics;
                entry.get_mut().with_lyrics = new_bool;
                new_bool
            }
            // Entry is necessarily occupied due to authority check
            Entry::Vacant(_) => unreachable!(),
        }
    };
    {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql.update_guild_lyrics(guild_id.0, new_bool).await {
            Ok(_) => debug!("Updated lyrics for guild id {}", guild_id.0),
            Err(why) => warn!("Could not set lyrics of guild: {}", why),
        }
    }

    let content = if new_bool {
        "Song commands can now be used in this server".to_string()
    } else {
        "Song commands can no longer be used in this server".to_string()
    };
    msg.channel_id
        .say(ctx, content)
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}
