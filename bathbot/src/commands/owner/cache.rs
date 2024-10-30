use bathbot_util::{numbers::WithComma, EmbedBuilder, FooterBuilder, MessageBuilder};
use eyre::Result;

use crate::{
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub async fn cache(command: InteractionCommand) -> Result<()> {
    let mut stats = Context::cache().stats();

    let guilds = stats.guilds().await?;
    let unavailable_guilds = stats.unavailable_guilds().await?;
    let users = stats.users().await?;
    let roles = stats.roles().await?;
    let channels = stats.channels().await?;

    let description = format!(
        "Guilds: {guilds}\n\
        Unavailable guilds: {unavailable_guilds}\n\
        Users: {users}\n\
        Roles: {roles}\n\
        Channels: {channels}",
        guilds = WithComma::new(guilds),
        unavailable_guilds = WithComma::new(unavailable_guilds),
        users = WithComma::new(users),
        roles = WithComma::new(roles),
        channels = WithComma::new(channels),
    );

    let embed = EmbedBuilder::new()
        .description(description)
        .footer(FooterBuilder::new("Boot time"))
        .timestamp(Context::get().start_time);

    let builder = MessageBuilder::new().embed(embed);
    command.callback(builder, false).await?;

    Ok(())
}
