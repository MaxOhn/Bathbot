use crate::{
    arguments::Args,
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu::models::GameMode;
use std::sync::Arc;
use twilight_model::channel::Message;

#[command]
#[authority()]
#[short_desc("Untrack all users in a channel")]
#[long_desc(
    "Stop notifying a channel about new plays in any user's top100.\n\
    If you only want to untrack all users of a single mode, \
    provide the mode as argument."
)]
#[usage("[osu / mania / taiko / ctb]")]
#[example("", "mania")]
async fn untrackall(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let mode = match args.next() {
        Some("osu") | Some("o") | Some("standard") | Some("s") => Some(GameMode::STD),
        Some("mania") | Some("m") => Some(GameMode::MNA),
        Some("taiko") | Some("t") => Some(GameMode::TKO),
        Some("ctb") | Some("c") => Some(GameMode::CTB),
        None => None,
        _ => {
            let content = "If an argument is provided, \
                it must be either `osu`, `mania`, `taiko`, or `ctb`.";
            return msg.error(&ctx, content).await;
        }
    };
    match ctx
        .tracking()
        .remove_channel(msg.channel_id, mode, ctx.psql())
        .await
    {
        Ok(amount) => {
            let content = format!("Untracked {} users in this channel", amount);
            msg.respond(&ctx, content).await
        }
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            Err(why)
        }
    }
}
