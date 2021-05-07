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

use crate::{util::MessageExt, BotResult, Context};

use std::sync::Arc;
use tokio::time::{interval, Duration};
use twilight_model::channel::Message;

async fn song_send(lyrics: &[&str], delay: u64, ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let allow = msg
        .guild_id
        .map_or(true, |guild_id| ctx.config_lyrics(guild_id));

    if allow {
        let mut interval = interval(Duration::from_millis(delay));
        for line in lyrics {
            interval.tick().await;

            ctx.http
                .create_message(msg.channel_id)
                .content(format!("♫ {} ♫", line))?
                .await?;
        }
    } else {
        let content = "The server's big boys disabled song commands. \
            Server authorities can re-enable them with the `lyrics` command";

        msg.error(&ctx, content).await?;
    }

    Ok(())
}
