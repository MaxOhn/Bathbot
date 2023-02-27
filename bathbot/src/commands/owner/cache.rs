use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    sync::Arc,
};

use bathbot_util::{numbers::WithComma, EmbedBuilder, FooterBuilder, MessageBuilder};
use eyre::Result;

use crate::{
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub async fn cache(ctx: Arc<Context>, command: InteractionCommand) -> Result<()> {
    let stats = ctx.cache.stats();

    let description = format!(
        "Guilds: {guilds}\n\
        Unavailable guilds: {unavailable_guilds}\n\
        Users: {users}\n\
        Roles: {roles}\n\
        Channels: {channels}",
        guilds = CacheValue::new(stats.guilds().await),
        unavailable_guilds = CacheValue::new(stats.unavailable_guilds().await),
        users = CacheValue::new(stats.users().await),
        roles = CacheValue::new(stats.roles().await),
        channels = CacheValue::new(stats.channels().await),
    );

    let embed = EmbedBuilder::new()
        .description(description)
        .footer(FooterBuilder::new("Boot time"))
        .timestamp(ctx.stats.start_time)
        .build();

    let builder = MessageBuilder::new().embed(embed);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}

struct CacheValue {
    value: Result<usize>,
}

impl CacheValue {
    fn new(value: Result<usize>) -> Self {
        Self { value }
    }
}

impl Display for CacheValue {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.value {
            Ok(ref n) => <WithComma<usize> as Display>::fmt(&WithComma::new(*n), f),
            Err(ref err) => {
                warn!("{err:?}");

                f.write_str("N/A")
            }
        }
    }
}
