mod bigger;
mod hint;
mod rankings;
mod start;
mod stop;
mod tags;

use std::sync::Arc;

use twilight_model::channel::Reaction;

use crate::{
    embeds::{BGHelpEmbed, EmbedData},
    util::{constants::common_literals::HELP, MessageExt},
    BotResult, CommandData, Context,
};

pub use self::{bigger::*, hint::*, rankings::*, start::*, stop::*, tags::*};

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
                let prefix = ctx.guild_first_prefix(msg.guild_id).await;

                let content =
                    format!("That's not a valid subcommand. Check `{prefix}bg` for more help.");

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
