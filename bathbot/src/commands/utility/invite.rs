use bathbot_macros::{SlashCommand, command};
use bathbot_util::{
    EmbedBuilder, FooterBuilder, MessageBuilder,
    constants::{BATHBOT_WORKSHOP, INVITE_LINK},
};
use eyre::Result;
use twilight_interactions::command::CreateCommand;

use crate::{core::commands::CommandOrigin, util::interaction::InteractionCommand};

#[derive(CreateCommand, SlashCommand)]
#[command(name = "invite", desc = "Invite me to your server")]
#[flags(SKIP_DEFER)]
pub struct Invite;

#[command]
#[desc("Invite me to your server")]
#[alias("inv")]
#[flags(SKIP_DEFER)]
#[group(Utility)]
async fn prefix_invite(msg: &Message) -> Result<()> {
    invite(msg.into()).await
}

pub async fn slash_invite(mut command: InteractionCommand) -> Result<()> {
    invite((&mut command).into()).await
}

async fn invite(orig: CommandOrigin<'_>) -> Result<()> {
    let embed = EmbedBuilder::new()
        .description(INVITE_LINK)
        .footer(FooterBuilder::new("The initial prefix will be <"))
        .title("Invite me to your server!");

    let builder = MessageBuilder::new().content(BATHBOT_WORKSHOP).embed(embed);
    orig.callback(builder).await?;

    Ok(())
}
