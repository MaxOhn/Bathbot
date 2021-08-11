use crate::{
    tracking::OSU_TRACKING_COOLDOWN, util::MessageExt, BotResult, CommandData, Context,
    MessageBuilder,
};

use std::{str::FromStr, sync::Arc};

#[command]
#[short_desc("Adjust the tracking cooldown (in ms) - default 5000")]
#[owner()]
async fn trackingcooldown(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (msg, mut args) = match data {
        CommandData::Message { msg, args, .. } => (msg, args),
        CommandData::Interaction { .. } => unreachable!(),
    };

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

    let builder = MessageBuilder::new().embed(content);
    msg.create_message(&ctx, builder).await?;

    Ok(())
}
