#![allow(unused_variables)]

mod bigger;
mod hint;
mod start;
mod stop;

pub use bigger::*;
pub use hint::*;
pub use start::*;
pub use stop::*;

use crate::{Args, BotResult, Context};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Play the background guessing game")]
#[aliases("bg")]
#[sub_commands(start, bigger, hint, stop)]
pub async fn backgroundgame(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    Ok(())
}
