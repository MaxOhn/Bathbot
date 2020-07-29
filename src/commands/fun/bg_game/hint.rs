use crate::{
    bail,
    util::{constants::GENERAL_ISSUE, error::BgGameError, MessageExt},
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Get a hint for the current background")]
#[aliases("h", "tip")]
#[bucket("bg_hint")]
pub async fn hint(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    match ctx.game_hint(msg.channel_id).await {
        Ok(hint) => msg.respond(&ctx, hint).await,
        Err(BgGameError::NotStarted) => {
            debug!("Could not get hint because game didn't start yet");
            Ok(())
        }
        Err(BgGameError::NoGame) => {
            let prefix = ctx.config_first_prefix(msg.guild_id);
            let content = format!(
                "No running game in this channel.\nStart one with `{}bg start`.",
                prefix
            );
            msg.error(&ctx, content).await
        }
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            bail!("Error while getting hint: {}", why);
        }
    }
}
