#![allow(unused_variables)]
mod bigger;
mod hint;
mod rankings;
mod start;
mod stop;

pub use bigger::*;
pub use hint::*;
pub use rankings::*;
pub use start::*;
pub use stop::*;

use crate::{
    embeds::{BGHelpEmbed, EmbedData},
    util::MessageExt,
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Play the background guessing game")]
#[aliases("bg")]
#[sub_commands(start, bigger, hint, stop, rankings)]
pub async fn backgroundgame(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    match args.next() {
        None | Some("help") => {
            let prefix = ctx.config_first_prefix(msg.guild_id);
            let embed = BGHelpEmbed::new(prefix).build().build();
            msg.build_response(&ctx, |m| m.embed(embed)).await
        }
        _ => {
            let prefix = ctx.config_first_prefix(msg.guild_id);
            let content = "That's not a valid subcommand. Check `{}bg` for more help.";
            msg.respond(&ctx, content).await
        }
    }
}
