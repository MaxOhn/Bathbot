use std::sync::Arc;

use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    commands::{
        utility::{config_, ConfigArgs},
        MyCommand,
    },
    util::{constants::INVITE_LINK, MessageExt},
    BotResult, CommandData, Context,
};

#[command]
#[short_desc("Deprecated command, use the slash command `/link` instead")]
async fn link(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, .. } => {
            let content = format!(
                "This command is deprecated and no longer works.\n\
                Use the slash command `/link` instead (no need to specify your osu! name).\n\
                If slash commands are not available in your server, \
                try [re-inviting the bot]({INVITE_LINK})."
            );

            return msg.error(&ctx, content).await;
        }
        CommandData::Interaction { command } => slash_link(ctx, *command).await,
    }
}

pub async fn slash_link(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    let mut args = ConfigArgs::default();
    args.osu = Some(true);

    config_(ctx, command, args).await
}

pub fn define_link() -> MyCommand {
    let help = "Link your discord to an osu! profile.\n\
        To unlink, use the `/config` command.\n\
        To link your discord to a twitch account you can also use the `/config` command.";

    MyCommand::new("link", "Link your discord to an osu! profile")
        .help(help)
        .options(Vec::new())
}
