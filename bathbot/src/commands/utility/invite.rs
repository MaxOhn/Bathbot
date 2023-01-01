use std::sync::Arc;

use bathbot_macros::{command, SlashCommand};
use eyre::Result;
use twilight_interactions::command::CreateCommand;

use crate::{
    core::commands::CommandOrigin,
    util::{
        builder::{EmbedBuilder, FooterBuilder, MessageBuilder},
        constants::{BATHBOT_WORKSHOP, INVITE_LINK},
        interaction::InteractionCommand,
    },
    Context,
};

#[derive(CreateCommand, SlashCommand)]
#[command(name = "invite")]
#[flags(SKIP_DEFER)]
/// Invite me to your server
pub struct Invite;

#[command]
#[desc("Invite me to your server")]
#[alias("inv")]
#[flags(SKIP_DEFER)]
#[group(Utility)]
async fn prefix_invite(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    invite(ctx, msg.into()).await
}

pub async fn slash_invite(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    invite(ctx, (&mut command).into()).await
}

async fn invite(ctx: Arc<Context>, orig: CommandOrigin<'_>) -> Result<()> {
    let embed = EmbedBuilder::new()
        .description(INVITE_LINK)
        .footer(FooterBuilder::new("The initial prefix will be <"))
        .title("Invite me to your server!")
        .build();

    let builder = MessageBuilder::new().content(BATHBOT_WORKSHOP).embed(embed);
    orig.callback(&ctx, builder).await?;

    Ok(())
}
