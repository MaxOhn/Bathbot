mod bombsaway;
mod catchit;
mod ding;
mod fireandflames;
mod fireflies;
mod flamingo;
mod pretender;
mod rockefeller;
mod saygoodbye;
mod tijdmachine;

pub use bombsaway::*;
pub use catchit::*;
pub use ding::*;
pub use fireandflames::*;
pub use fireflies::*;
pub use flamingo::*;
pub use pretender::*;
pub use rockefeller::*;
pub use saygoodbye::*;
pub use tijdmachine::*;

use crate::{
    bail,
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, Context,
};

use std::sync::Arc;
use tokio::time;
use twilight::model::channel::Message;

async fn song_send(lyrics: &[&str], delay: u64, ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let allow = match msg.guild_id {
        Some(guild_id) => match ctx.guilds().get(&guild_id) {
            Some(config) => config.with_lyrics,
            None => {
                msg.respond(&ctx, GENERAL_ISSUE).await?;
                bail!("No config for guild {}", guild_id);
            }
        },
        None => true,
    };
    if allow {
        let mut interval = time::interval(time::Duration::from_millis(delay));
        for line in lyrics {
            interval.tick().await;
            ctx.http
                .create_message(msg.channel_id)
                .content(format!("♫ {} ♫", line))?
                .await?;
        }
    } else {
        let guard = ctx.guilds().get(&msg.guild_id.unwrap()).unwrap();
        let prefix = &guard.value().prefixes[0];
        let content = format!(
            "The server's big boys disabled song commands. \
                Server authorities can re-enable them by typing `{}lyrics`",
            prefix
        );
        msg.respond(&ctx, content).await?;
    }
    Ok(())
}
