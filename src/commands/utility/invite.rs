use crate::{
    embeds::{EmbedData, InviteEmbed},
    util::{constants::BATHBOT_WORKSHOP, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;
use twilight_model::application::{command::Command, interaction::ApplicationCommand};

#[command]
#[short_desc("Invite me to your server")]
#[aliases("inv")]
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
    Command {
        application_id: None,
        guild_id: None,
        name: "invite".to_owned(),
        default_permission: None,
        description: "Invite me to your server".to_owned(),
        id: None,
        options: Vec::new(),
    }
}
