use std::sync::Arc;

use crate::{
    util::{
        builder::{EmbedBuilder, FooterBuilder, MessageBuilder},
        interaction::InteractionCommand,
        numbers::with_comma_int,
        InteractionCommandExt,
    },
    BotResult, Context,
};

pub async fn cache(ctx: Arc<Context>, command: InteractionCommand) -> BotResult<()> {
    let stats = ctx.cache.stats();

    let description = format!(
        "Guilds: {guilds}\n\
        Members: {members}\n\
        Users: {users}\n\
        Roles: {roles}\n\
        Channels: {channels}",
        guilds = with_comma_int(stats.guilds()),
        members = with_comma_int(stats.members()),
        users = with_comma_int(stats.users()),
        roles = with_comma_int(stats.roles()),
        channels = with_comma_int(stats.channels_total()),
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
