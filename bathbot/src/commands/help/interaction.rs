use std::collections::BTreeMap;

use bathbot_macros::SlashCommand;
use bathbot_util::{
    constants::{BATHBOT_GITHUB, BATHBOT_ROADMAP, BATHBOT_WORKSHOP, INVITE_LINK, KOFI},
    datetime::HowLongAgoDynamic,
    numbers::WithComma,
    string_cmp::levenshtein_distance,
    CowUtils, EmbedBuilder, MessageBuilder,
};
use eyre::{ContextCompat, Result};
use metrics::Key;
use twilight_interactions::command::{AutocompleteValue, CommandModel, CreateCommand};
use twilight_model::{
    application::command::{CommandOptionChoice, CommandOptionChoiceValue},
    channel::message::embed::EmbedField,
};

use super::failed_message_content;
use crate::{
    active::{impls::HelpInteractionCommand, ActiveMessages},
    core::{
        commands::interaction::{
            twilight_command::Command, InteractionCommandKind, InteractionCommands,
        },
        Context,
    },
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

#[derive(CreateCommand, SlashCommand)]
#[flags(SKIP_DEFER)]
#[command(
    name = "help",
    desc = "Display general help or help for a specific command"
)]
#[allow(dead_code)]
pub struct Help {
    #[command(autocomplete = true, desc = "Specify a command base name")]
    command: Option<String>,
}

#[derive(CommandModel)]
#[command(autocomplete = true)]
struct Help_ {
    command: AutocompleteValue<String>,
}

pub async fn slash_help(mut command: InteractionCommand) -> Result<()> {
    let args = Help_::from_interaction(command.input_data())?;

    match args.command {
        AutocompleteValue::None => help_slash_basic(command).await,
        AutocompleteValue::Completed(name) => match InteractionCommands::get().command(&name) {
            Some(cmd) => help_slash_command(&mut command, cmd).await,
            None => {
                let dists: BTreeMap<_, _> = InteractionCommands::get()
                    .names()
                    .map(|cmd| (levenshtein_distance(&name, cmd).0, cmd))
                    .filter(|(dist, _)| *dist < 5)
                    .collect();

                let content = failed_message_content(dists);
                command.error_callback(content).await?;

                Ok(())
            }
        },
        AutocompleteValue::Focused(name) => {
            let name = name.cow_to_ascii_lowercase();
            let arg = name.trim();

            let choices = match (arg, InteractionCommands::get().descendants(arg)) {
                ("", _) | (_, None) => Vec::new(),
                (_, Some(cmds)) => cmds
                    .map(|cmd| CommandOptionChoice {
                        name: cmd.to_owned(),
                        name_localizations: None,
                        value: CommandOptionChoiceValue::String(cmd.to_owned()),
                    })
                    .collect(),
            };

            command.autocomplete(choices).await?;

            Ok(())
        }
    }
}

async fn help_slash_basic(command: InteractionCommand) -> Result<()> {
    let cache = Context::cache();

    let id = cache
        .current_user()
        .await?
        .wrap_err("Missing CurrentUser in cache")?
        .id;

    let mention = format!("<@{id}>");

    let description = format!(
        "{mention} is a discord bot written by [Badewanne3](https://osu.ppy.sh/u/2211396) all around osu!"
    );

    let join_server = EmbedField {
        inline: false,
        name: "Got a question, suggestion, bug, or are interested in the development?".to_owned(),
        value: format!(
            "Feel free to join the [discord server]({BATHBOT_WORKSHOP}).\n\
            [This roadmap]({BATHBOT_ROADMAP}) shows already suggested features and known bugs.",
        ),
    };

    let command_help = EmbedField {
        inline: false,
        name: "Want to learn more about a command?".to_owned(),
        value: "Try specifying the command name on the `help` command: `/help command:_`"
            .to_owned(),
    };

    let invite = EmbedField {
        inline: false,
        name: "Want to invite the bot to your server?".to_owned(),
        value: format!("Try using this [**invite link**]({INVITE_LINK})"),
    };

    let mut stats = cache.stats();

    let guilds = match stats.guilds().await {
        Ok(guilds) => guilds,
        Err(err) => {
            warn!(?err, "Failed to fetch guild count");

            0
        }
    };

    let unavailable_guilds = match stats.unavailable_guilds().await {
        Ok(unavailable_guilds) => unavailable_guilds,
        Err(err) => {
            warn!(?err, "Failed to fetch unavailable guild count");

            0
        }
    };

    let servers = EmbedField {
        inline: true,
        name: "Servers".to_owned(),
        value: WithComma::new(guilds + unavailable_guilds).to_string(),
    };

    let ctx = Context::get();
    let boot_time = ctx.start_time;

    let boot_up = EmbedField {
        inline: true,
        name: "Boot-up".to_owned(),
        value: HowLongAgoDynamic::new(&boot_time).to_string(),
    };

    let github = EmbedField {
        inline: false,
        name: "Interested in the code?".to_owned(),
        value: format!("The source code can be found over at [github]({BATHBOT_GITHUB})"),
    };

    let commands_used = ctx
        .metrics
        .sum_counters(&Key::from_static_name("bathbot.commands_process_time"));

    let commands_used = EmbedField {
        inline: true,
        name: "Commands used".to_owned(),
        value: WithComma::new(commands_used).to_string(),
    };

    let key = Key::from_static_name("bathbot.osu_response_time");
    let osu_requests = ctx.metrics.sum_histograms(&key);

    let osu_requests = EmbedField {
        inline: true,
        name: "osu!api requests".to_owned(),
        value: WithComma::new(osu_requests).to_string(),
    };

    let kofi = EmbedField {
        inline: false,
        name: "Feel like supporting the bot's development & maintenance?".to_owned(),
        value: format!("Donations through [Ko-fi]({KOFI}) are very much appreciated <3"),
    };

    let fields = vec![
        join_server,
        command_help,
        invite,
        servers,
        boot_up,
        github,
        commands_used,
        osu_requests,
        kofi,
    ];

    let embed = EmbedBuilder::new().description(description).fields(fields);

    let builder = MessageBuilder::new().embed(embed);

    command.callback(builder, true).await?;

    Ok(())
}

async fn help_slash_command(
    command: &mut InteractionCommand,
    cmd: InteractionCommandKind,
) -> Result<()> {
    let Command { name, .. } = cmd.create();

    if name == "owner" {
        let description =
            "This command can only be used by the owner of the bot.\nQuit snooping around :^)";

        let embed_builder = EmbedBuilder::new().title(name).description(description);
        let builder = MessageBuilder::new().embed(embed_builder);
        command.callback(builder, true).await?;

        return Ok(());
    }

    let help = HelpInteractionCommand::new(name, command.user_id()?);

    ActiveMessages::builder(help).begin(command).await
}
