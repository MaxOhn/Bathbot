use bathbot_macros::{command, SlashCommand};
use bathbot_util::constants::INVITE_LINK;
use eyre::Result;
use twilight_interactions::command::CreateCommand;

use crate::{
    commands::utility::{config, Config, ConfigLink},
    util::{interaction::InteractionCommand, ChannelExt},
};

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "link",
    desc = "Link your discord to an osu! profile",
    help = "Link your discord to an osu! profile.\n\
    To unlink, use the `/config` command.\n\
    To link your discord to a twitch account you can also use the `/config` command."
)]
#[flags(EPHEMERAL)]
pub struct Link;

async fn slash_link(command: InteractionCommand) -> Result<()> {
    let mut args = Config::default();
    args.osu = Some(ConfigLink::Link);

    config(command, args).await
}

#[command]
#[desc("Deprecated command, use the slash command `/link` instead")]
#[flags(SKIP_DEFER)]
#[group(AllModes)]
async fn prefix_link(msg: &Message) -> Result<()> {
    let content = format!(
        "This command is deprecated and no longer works.\n\
        Use the slash command `/link` instead (no need to specify your osu! name).\n\
        If slash commands are not available in your server, \
        try [re-inviting the bot]({INVITE_LINK})."
    );

    msg.error(content).await?;

    Ok(())
}
