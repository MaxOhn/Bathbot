use std::sync::Arc;

use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    util::{builder::MessageBuilder, ApplicationCommandExt},
    BotResult, Context,
};

pub async fn trackingcooldown(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    ms: f32,
) -> BotResult<()> {
    let previous = ctx.tracking().set_cooldown(ms);

    let content = format!(
        "Tracking cooldown: {}ms -> {}ms",
        previous as u32, ms as u32
    );

    let builder = MessageBuilder::new().embed(content);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}
