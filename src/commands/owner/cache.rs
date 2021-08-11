use crate::{
    embeds::{CacheEmbed, EmbedData},
    util::MessageExt,
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;
use twilight_model::application::{command::Command, interaction::ApplicationCommand};

#[command]
#[short_desc("Display stats about the internal cache")]
#[owner()]
async fn cache(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let stats = ctx.cache.stats();

    let embed = CacheEmbed::new(stats, ctx.stats.start_time)
        .into_builder()
        .build();

    let builder = MessageBuilder::new().embed(embed);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

pub async fn slash_cache(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    cache(ctx, command.into()).await
}

pub fn _slash_cache_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "cache".to_owned(),
        default_permission: None,
        description: "Display stats about the internal cache".to_owned(),
        id: None,
        options: vec![],
    }
}
