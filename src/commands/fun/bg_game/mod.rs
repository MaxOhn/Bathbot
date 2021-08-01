mod bigger;
mod hint;
mod rankings;
mod start;
mod stop;
mod tags;

pub use bigger::*;
pub use hint::*;
pub use rankings::*;
pub use start::*;
pub use stop::*;
pub use tags::*;

use crate::{
    embeds::{BGHelpEmbed, EmbedData},
    util::MessageExt,
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight_model::channel::{Message, Reaction};

#[command]
#[short_desc("Play the background guessing game")]
#[long_desc(
    "Play the background guessing game.\n\
    Use this command without arguments to see the full help."
)]
#[aliases("bg")]
#[sub_commands(start, bigger, hint, stop, rankings)]
pub async fn backgroundgame(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    match args.next() {
        None | Some("help") => {
            let prefix = ctx.config_first_prefix(msg.guild_id);
            let embed = &[BGHelpEmbed::new(prefix).into_builder().build()];

            msg.build_response(&ctx, |m| m.embeds(embed)).await
        }
        _ => {
            let prefix = ctx.config_first_prefix(msg.guild_id);

            let content = format!(
                "That's not a valid subcommand. Check `{}bg` for more help.",
                prefix
            );

            msg.error(&ctx, content).await
        }
    }
}

enum ReactionWrapper {
    Add(Reaction),
    Remove(Reaction),
}

impl ReactionWrapper {
    #[inline]
    fn as_deref(&self) -> &Reaction {
        match self {
            Self::Add(r) | Self::Remove(r) => r,
        }
    }
}
