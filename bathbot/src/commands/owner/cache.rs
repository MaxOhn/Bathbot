use std::sync::Arc;

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
        Members: {members}\n\
        Users: {users}\n\
        Roles: {roles}\n\
        Channels: {channels}",
        guilds = WithComma::new(stats.guilds()),
        members = WithComma::new(stats.members()),
        users = WithComma::new(stats.users()),
        roles = WithComma::new(stats.roles()),
        channels = WithComma::new(stats.channels_total()),
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
