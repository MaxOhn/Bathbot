use crate::{
    tracking::OSU_TRACKING_COOLDOWN, util::MessageExt, BotResult, CommandData, Context,
    MessageBuilder,
};

use std::{str::FromStr, sync::Arc};

#[command]
#[short_desc("Adjust the tracking cooldown (in ms) - default 5000")]
#[owner()]
async fn trackingcooldown(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let ms = match args.next().map(f32::from_str) {
                Some(Ok(value)) => value,
                Some(Err(_)) => return msg.error(&ctx, "Expected f32 as first argument").await,
                None => *OSU_TRACKING_COOLDOWN,
            };

            _trackingcooldown(ctx, CommandData::Message { msg, args, num }, ms).await
        }
        CommandData::Interaction { command } => super::slash_owner(ctx, command).await,
    }
}

pub(super) async fn _trackingcooldown(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    ms: f32,
) -> BotResult<()> {
    let previous = ctx.tracking().set_cooldown(ms).await;

    let content = format!(
        "Tracking cooldown: {}ms -> {}ms",
        previous as u32, ms as u32
    );

    let builder = MessageBuilder::new().embed(content);
    data.create_message(&ctx, builder).await?;

    Ok(())
}
