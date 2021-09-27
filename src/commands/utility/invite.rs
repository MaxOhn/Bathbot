use crate::{
    commands::SlashCommandBuilder,
    embeds::{EmbedData, InviteEmbed},
    util::{constants::BATHBOT_WORKSHOP, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;
use twilight_model::application::{command::Command, interaction::ApplicationCommand};

#[command]
#[short_desc("Invite me to your server")]
#[aliases("inv")]
#[no_typing()]
async fn invite(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let embed = InviteEmbed::new().into_builder().build();
    let builder = MessageBuilder::new().content(BATHBOT_WORKSHOP).embed(embed);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

pub async fn slash_invite(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    invite(ctx, command.into()).await
}

pub fn slash_invite_command() -> Command {
    SlashCommandBuilder::new("invite", "Invite me to your server").build()
}
