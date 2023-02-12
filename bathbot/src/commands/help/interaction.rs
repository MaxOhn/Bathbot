use std::{collections::BTreeMap, sync::Arc};

use bathbot_macros::SlashCommand;
use bathbot_util::{
    constants::{BATHBOT_GITHUB, BATHBOT_ROADMAP, BATHBOT_WORKSHOP, INVITE_LINK, KOFI},
    datetime::HowLongAgoDynamic,
    numbers::WithComma,
    string_cmp::levenshtein_distance,
    CowUtils, EmbedBuilder, FooterBuilder, MessageBuilder,
};
use eyre::Result;
use prometheus::core::Collector;
use twilight_interactions::command::{
    ApplicationCommandData, AutocompleteValue, CommandModel, CreateCommand,
};
use twilight_model::{application::command::CommandOptionChoice, channel::embed::EmbedField};

use crate::{
    core::{
        commands::slash::{SlashCommand, SlashCommands},
        Context,
    },
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

use super::{failed_message_content, option_fields, parse_select_menu, AUTHORITY_STATUS};

#[derive(CreateCommand, SlashCommand)]
#[flags(SKIP_DEFER)]
#[command(name = "help")]
#[allow(dead_code)]
/// Display general help or help for a specific command
pub struct Help {
    #[command(autocomplete = true)]
    /// Specify a command base name
    command: Option<String>,
}

#[derive(CommandModel)]
#[command(autocomplete = true)]
struct Help_ {
    command: AutocompleteValue<String>,
}

pub async fn slash_help(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Help_::from_interaction(command.input_data())?;

    match args.command {
        AutocompleteValue::None => help_slash_basic(ctx, command).await,
        AutocompleteValue::Completed(name) => match SlashCommands::get().command(&name) {
            Some(cmd) => help_slash_command(&ctx, command, cmd).await,
            None => {
                let dists: BTreeMap<_, _> = SlashCommands::get()
                    .names()
                    .map(|cmd| (levenshtein_distance(&name, cmd).0, cmd))
                    .filter(|(dist, _)| *dist < 5)
                    .collect();

                let content = failed_message_content(dists);
                command.error_callback(&ctx, content).await?;

                Ok(())
            }
        },
        AutocompleteValue::Focused(name) => {
            let name = name.cow_to_ascii_lowercase();
            let arg = name.trim();

            let choices = match (arg, SlashCommands::get().descendants(arg)) {
                ("", _) | (_, None) => Vec::new(),
                (_, Some(cmds)) => cmds
                    .map(|cmd| CommandOptionChoice::String {
                        name: cmd.to_owned(),
                        name_localizations: None,
                        value: cmd.to_owned(),
                    })
                    .collect(),
            };

            command.autocomplete(&ctx, choices).await?;

            Ok(())
        }
    }
}

async fn help_slash_basic(ctx: Arc<Context>, command: InteractionCommand) -> Result<()> {
    let id = ctx
        .cache
        .current_user(|user| user.id)
        .expect("missing CurrentUser in cache");

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

    let servers = EmbedField {
        inline: true,
        name: "Servers".to_owned(),
        value: WithComma::new(ctx.cache.stats().guilds()).to_string(),
    };

    let boot_time = ctx.stats.start_time;

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

    let commands_used: usize = ctx.stats.command_counts.message_commands.collect()[0]
        .get_metric()
        .iter()
        .map(|metrics| metrics.get_counter().get_value() as usize)
        .sum();

    let commands_used = EmbedField {
        inline: true,
        name: "Commands used".to_owned(),
        value: WithComma::new(commands_used).to_string(),
    };

    let osu_requests: usize = ctx.stats.osu_metrics.rosu.collect()[0]
        .get_metric()
        .iter()
        .map(|metric| metric.get_counter().get_value() as usize)
        .sum();

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

    let embed = EmbedBuilder::new()
        .description(description)
        .fields(fields)
        .build();

    let builder = MessageBuilder::new().embed(embed);

    command.callback(&ctx, builder, true).await?;

    Ok(())
}

async fn help_slash_command(
    ctx: &Context,
    command: InteractionCommand,
    cmd: &SlashCommand,
) -> Result<()> {
    let ApplicationCommandData {
        name,
        description,
        help,
        options,
        ..
    } = (cmd.create)();

    if name == "owner" {
        let description =
            "This command can only be used by the owner of the bot.\nQuit snooping around :^)";

        let embed_builder = EmbedBuilder::new().title(name).description(description);
        let builder = MessageBuilder::new().embed(embed_builder);
        command.callback(ctx, builder, true).await?;

        return Ok(());
    }

    let description = help.unwrap_or(description);

    let mut embed_builder = EmbedBuilder::new()
        .title(name)
        .description(description)
        .fields(option_fields(&options));

    if cmd.flags.authority() {
        let footer = FooterBuilder::new(AUTHORITY_STATUS);
        embed_builder = embed_builder.footer(footer);
    }

    let menu = parse_select_menu(&options);

    let builder = MessageBuilder::new()
        .embed(embed_builder)
        .components(menu.unwrap_or_default());

    command.callback(ctx, builder, true).await?;

    Ok(())
}
