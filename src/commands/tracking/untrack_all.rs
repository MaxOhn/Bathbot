use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use rosu_v2::model::GameMode;
use std::sync::Arc;

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
async fn untrackall(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let mode = match args.next() {
                Some("osu") | Some("o") | Some("standard") | Some("s") => Some(GameMode::STD),
                Some("mania") | Some("m") => Some(GameMode::MNA),
                Some("taiko") | Some("t") => Some(GameMode::TKO),
                Some("ctb") | Some("c") | Some("catch") => Some(GameMode::CTB),
                None => None,
                _ => {
                    let content = "If an argument is provided, \
                        it must be either `osu`, `mania`, `taiko`, or `ctb`.";

                    return msg.error(&ctx, content).await;
                }
            };

            _untrackall(ctx, CommandData::Message { msg, args, num }, mode).await
        }
        CommandData::Interaction { command } => super::slash_track(ctx, *command).await,
    }
}

pub async fn _untrackall(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mode: Option<GameMode>,
) -> BotResult<()> {
    let channel_id = data.channel_id();
    let remove_fut = ctx.tracking().remove_channel(channel_id, mode, ctx.psql());

    match remove_fut.await {
        Ok(amount) => {
            let content = format!("Untracked {} users in this channel", amount);
            let builder = MessageBuilder::new().content(content);
            data.create_message(&ctx, builder).await?;

            Ok(())
        }
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            Err(why)
        }
    }
}
