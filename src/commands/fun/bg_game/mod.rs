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
    util::{constants::common_literals::HELP, MessageExt},
    BotResult, CommandData, Context,
};

use std::sync::Arc;
use twilight_model::channel::Reaction;

#[command]
#[short_desc("Play the background guessing game")]
#[long_desc(
    "Play the background guessing game.\n\
    Use this command without arguments to see the full help."
)]
#[aliases("bg")]
#[sub_commands(start, bigger, hint, stop, rankings)]
pub async fn backgroundgame(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, .. } => match args.next() {
            None | Some(HELP) => {
                let builder = BGHelpEmbed::new().into_builder().build().into();
                msg.create_message(&ctx, builder).await?;

                Ok(())
            }
            _ => {
                let prefix = ctx.config_first_prefix(msg.guild_id).await;

                let content = format!(
                    "That's not a valid subcommand. Check `{}bg` for more help.",
                    prefix
                );

                msg.error(&ctx, content).await
            }
        },
        CommandData::Interaction { .. } => unreachable!(),
    }
}

enum ReactionWrapper {
    Add(Reaction),
    Remove(Reaction),
}

impl ReactionWrapper {
    fn as_deref(&self) -> &Reaction {
        match self {
            Self::Add(r) | Self::Remove(r) => r,
        }
    }
}
