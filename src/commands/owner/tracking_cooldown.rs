use crate::{tracking::OSU_TRACKING_COOLDOWN, util::MessageExt, Args, BotResult, Context};

use std::{str::FromStr, sync::Arc};
use twilight::model::channel::Message;

#[command]
#[short_desc("Adjust the tracking cooldown (in ms) - default 5000")]
#[owner()]
async fn trackingcooldown(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    let cooldown = match args.next().map(f32::from_str) {
        Some(Ok(value)) => value,
        Some(Err(_)) => return msg.error(&ctx, "Expected i64 as first argument").await,
        None => *OSU_TRACKING_COOLDOWN,
    };
    let previous = ctx.tracking().set_cooldown(cooldown).await;
    let content = format!(
        "Tracking cooldown: {}ms -> {}ms",
        previous as u32, cooldown as u32
    );
    msg.respond(&ctx, content).await?;
    Ok(())
}
