use crate::{
    commands::{
        utility::{config_, ConfigArgs},
        MyCommand, MyCommandOption,
    },
    util::{
        constants::{common_literals::OSU, INVITE_LINK},
        ApplicationCommandExt, MessageExt,
    },
    BotResult, CommandData, Context,
};

use std::sync::Arc;
use twilight_model::application::{
    command::CommandOptionChoice,
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

const LINK: &str = "link";

pub async fn slash_link(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut osu = None;
    let mut twitch = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, value } => match name.as_str() {
                OSU => osu = Some(value == LINK),
                "twitch" => twitch = Some(value == LINK),
                _ => bail_cmd_option!(LINK, string, name),
            },
            CommandDataOption::Integer { name, .. } => {
                bail_cmd_option!(LINK, integer, name)
            }
            CommandDataOption::Boolean { name, .. } => {
                bail_cmd_option!(LINK, boolean, name)
            }
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!(LINK, subcommand, name)
            }
        }
    }

    let mut args = ConfigArgs::default();
    args.osu = osu;
    args.twitch = twitch;

    config_(ctx, command, args).await
}

fn link_options() -> Vec<CommandOptionChoice> {
    let link = CommandOptionChoice::String {
        name: "Link".to_owned(),
        value: LINK.to_owned(),
    };

    let unlink = CommandOptionChoice::String {
        name: "Unlink".to_owned(),
        value: "unlink".to_owned(),
    };

    vec![link, unlink]
}

pub fn define_link() -> MyCommand {
    let osu_description = "Specify whether you want to link to an osu! profile";

    let osu_help = "Most osu! commands require a specified username to work.\n\
        Since using a command is most commonly intended for your own profile, you can link \
        your discord with an osu! profile so that when no username is specified in commands, \
        it will choose the linked username.\n\
        If the value is set to `Link`, it will prompt you to authorize your account.\n\
        If `Unlink` is selected, you will be unlinked from the osu! profile.";

    let osu = MyCommandOption::builder(OSU, osu_description)
        .help(osu_help)
        .string(link_options(), false);

    let twitch_description = "Specify whether you want to link to a twitch profile";

    let twitch_help = "With this option you can link to a twitch channel.\n\
        When you have both your osu! and twitch linked, are currently streaming, and anyone uses \
        the `recent score` command on your osu! username, it will try to retrieve the last VOD from your \
        twitch channel and link to a timestamp for the score.\n\
        If the value is set to `Link`, it will prompt you to authorize your account.\n\
        If `Unlink` is selected, you will be unlinked from the twitch channel.";

    let twitch = MyCommandOption::builder("twitch", twitch_description)
        .help(twitch_help)
        .string(link_options(), false);

    let description = "(Un)link your discord to an osu! or twitch account";
    let help = "This command allows you to link or unlink to an osu! or twitch account.";

    MyCommand::new(LINK, description)
        .help(help)
        .options(vec![osu, twitch])
}
