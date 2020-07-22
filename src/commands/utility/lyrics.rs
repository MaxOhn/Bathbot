use crate::{
    bail,
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, Context,
};

use std::{collections::hash_map::Entry, sync::Arc};
use twilight::model::channel::Message;

#[command]
// #[only_in("guild")]
// #[checks(Authority)]
#[short_desc("Toggle availability of song commands")]
#[long_desc(
    "Toggle whether song commands can be used in this server. \
    Defaults to `true`"
)]
async fn lyrics(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let guild_id = msg.guild_id.unwrap();
    let guilds = &ctx.guilds;
    let mut available = false;
    let config = guilds.update_get(&guild_id, |_, config| {
        let mut new_config = config.clone();
        new_config.with_lyrics = !config.with_lyrics;
        available = new_config.with_lyrics;
        new_config
    });
    if let Some(config) = config {
        let psql = &ctx.clients.psql;
        match psql.set_guild_config(guild_id.0, config.value()).await {
            Ok(_) => debug!("Updated lyrics for guild id {}", guild_id.0),
            Err(why) => warn!("Could not set lyrics of guild: {}", why),
        }
    } else {
        msg.respond(&ctx, GENERAL_ISSUE.to_owned()).await?;
        bail!("GuildId {} not found in guilds", guild_id);
    }

    let content = if available {
        "Song commands can now be used in this server"
    } else {
        "Song commands can no longer be used in this server"
    };
    msg.respond(&ctx, content.to_owned()).await?;
    Ok(())
}
