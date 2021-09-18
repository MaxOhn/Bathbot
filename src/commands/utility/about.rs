use crate::{
    commands::SlashCommandBuilder,
    embeds::{AboutEmbed, EmbedData},
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context,
};

use std::sync::Arc;
use twilight_model::application::{command::Command, interaction::ApplicationCommand};

#[command]
#[short_desc("Displaying some information about this bot")]
#[aliases("info")]
async fn about(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let embed_data = match AboutEmbed::new(&ctx).await {
        Ok(data) => data,
        Err(why) => {
            data.error(&ctx, GENERAL_ISSUE).await?;

            return Err(why);
        }
    };

    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}

pub async fn slash_about(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    about(ctx, command.into()).await
}

pub fn slash_about_command() -> Command {
    let description = "Displaying some information about this bot";

    SlashCommandBuilder::new("about", description).build()
}
