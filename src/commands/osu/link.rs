use crate::{
    commands::utility::{config_, ConfigArgs},
    util::{constants::INVITE_LINK, ApplicationCommandExt, MessageExt},
    BotResult, CommandData, Context,
};

use std::sync::Arc;
use twilight_model::application::{
    command::{BaseCommandOptionData, Command, CommandOption},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

#[command]
#[short_desc("Deprecated command, use the slash command `/link` instead")]
async fn link(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, .. } => {
            let content = format!(
                "This command is deprecated and no longer works.\n\
                Use the slash command `/link` instead.\n\
                If slash commands are not available in your server, \
                try [re-inviting the bot]({}).",
                INVITE_LINK
            );

            return msg.error(&ctx, content).await;
        }
        CommandData::Interaction { command } => slash_link(ctx, *command).await,
    }
}

pub async fn slash_link(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut osu = None;
    let mut twitch = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, .. } => {
                bail_cmd_option!("config", string, name)
            }
            CommandDataOption::Integer { name, .. } => {
                bail_cmd_option!("config", integer, name)
            }
            CommandDataOption::Boolean { name, value } => match name.as_str() {
                "osu" => osu = Some(value),
                "twitch" => twitch = Some(value),
                _ => bail_cmd_option!("config", boolean, name),
            },
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("config", subcommand, name)
            }
        }
    }

    let mut args = ConfigArgs::default();
    args.osu = osu;
    args.twitch = twitch;

    config_(ctx, command, args).await
}

pub fn slash_link_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "link".to_owned(),
        default_permission: None,
        description: "(Un)Link your discord to an osu! or twitch account".to_owned(),
        id: None,
        options: vec![
            CommandOption::Boolean(BaseCommandOptionData {
                description: "Specify whether you want to link to an osu! profile (choose `false` to unlink)".to_owned(),
                name: "osu".to_owned(),
                required: false,
            }),
            CommandOption::Boolean(BaseCommandOptionData {
                description: "Specify whether you want to link to a twitch channel (choose `false` to unlink)".to_owned(),
                name: "twitch".to_owned(),
                required: false,
            }),
        ],
    }
}
