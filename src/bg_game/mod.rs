mod game;
mod game_wrapper;
mod hints;
mod img_reveal;
mod tags;
mod util;

use game::{game_loop, Game, LoopResult};
pub use game_wrapper::GameWrapper;
use hints::Hints;
use img_reveal::ImageReveal;
pub use tags::MapsetTags;
use twilight_model::id::ChannelId;

use crate::{core::Context, error::BgGameError, BotResult};

type GameResult<T> = Result<T, BgGameError>;

async fn send_msg(ctx: &Context, channel: ChannelId, content: &str) -> BotResult<()> {
    ctx.http
        .create_message(channel)
        .content(content)?
        .exec()
        .await?;

    Ok(())
}
