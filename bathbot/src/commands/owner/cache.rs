use bathbot_util::{numbers::WithComma, EmbedBuilder, FooterBuilder, MessageBuilder};
use eyre::Result;

use crate::{
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub async fn cache(command: InteractionCommand) -> Result<()> {
    let stats = Context::cache().stats();

    let description = format!(
        "Guilds: {guilds}\n\
        Unavailable guilds: {unavailable_guilds}\n\
        Users: {users}\n\
        Roles: {roles}\n\
        Channels: {channels}",
        guilds = WithComma::new(stats.guilds),
        unavailable_guilds = WithComma::new(stats.unavailable_guilds),
        users = WithComma::new(stats.users),
        roles = WithComma::new(stats.roles),
        channels = WithComma::new(stats.channels),
    );

    let embed = EmbedBuilder::new()
        .description(description)
        .footer(FooterBuilder::new("Boot time"))
        .timestamp(Context::get().start_time);

    let builder = MessageBuilder::new().embed(embed);
    command.callback(builder, false).await?;

    Ok(())
}
