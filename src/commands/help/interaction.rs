use std::{collections::BTreeMap, sync::Arc};

use command_macros::SlashCommand;
use prometheus::core::Collector;
use twilight_interactions::{
    command::{ApplicationCommandData, CommandModel, CreateCommand},
    error::{ParseError, ParseOptionError, ParseOptionErrorType},
};
use twilight_model::{
    application::{
        command::CommandOptionChoice,
        interaction::{ApplicationCommand, ApplicationCommandAutocomplete},
    },
    channel::embed::EmbedField,
};

use crate::{
    core::{
        commands::slash::{SlashCommand, SLASH_COMMANDS},
        Context,
    },
    util::{
        builder::{EmbedBuilder, FooterBuilder, MessageBuilder},
        constants::{BATHBOT_GITHUB, BATHBOT_WORKSHOP, INVITE_LINK, KOFI},
        datetime::how_long_ago_dynamic,
        levenshtein_distance,
        numbers::with_comma_int,
        ApplicationCommandExt, AutocompleteExt, CowUtils,
    },
    BotResult,
};

use super::{failed_message_content, option_fields, parse_select_menu, AUTHORITY_STATUS};

pub async fn handle_help_autocomplete(
    ctx: Arc<Context>,
    command: Box<ApplicationCommandAutocomplete>,
) -> BotResult<()> {
    let mut cmd_name = None;

    if let Some(option) = command.data.options.first() {
        match option.value {
            Some(ref value) if option.name == "command" => cmd_name = Some(value),
            _ => {
                let err = ParseOptionError {
                    field: option.name.clone(),
                    kind: ParseOptionErrorType::RequiredField,
                };

                return Err(ParseError::Option(err).into());
            }
        }
    }

    let name = cmd_name.map(|name| name.cow_to_ascii_lowercase());

    let choices = match name {
        Some(name) => {
            let arg = name.trim();

            match (arg, SLASH_COMMANDS.descendants(arg)) {
                ("", _) | (_, None) => Vec::new(),
                (_, Some(cmds)) => cmds
                    .map(|cmd| CommandOptionChoice::String {
                        name: cmd.to_owned(),
                        value: cmd.to_owned(),
                    })
                    .collect(),
            }
        }
        _ => Vec::new(),
    };

    command.callback(&ctx, choices).await?;

    Ok(())
}

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "help")]
#[flags(SKIP_DEFER)]
/// Display general help or help for a specific command
pub struct Help {
    #[command(autocomplete = true)]
    /// Specify a command base name
    command: Option<String>,
}

async fn slash_help(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = Help::from_interaction(command.input_data())?;

    match args.command {
        Some(name) => match SLASH_COMMANDS.command(&name) {
            Some(cmd) => help_slash_command(&ctx, command, cmd).await,
            None => {
                let dists: BTreeMap<_, _> = SLASH_COMMANDS
                    .names()
                    .map(|cmd| (levenshtein_distance(&name, cmd).0, cmd))
                    .filter(|(dist, _)| *dist < 5)
                    .collect();

                let content = failed_message_content(dists);
                command.error_callback(&ctx, content).await?;

                Ok(())
            }
        },
        None => help_slash_basic(ctx, command).await,
    }
}

async fn help_slash_basic(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    let id = ctx
        .cache
        .current_user()
        .expect("missing CurrentUser in cache")
        .id;

    let mention = format!("<@{id}>");

    let description = format!(
        "{mention} is a discord bot written by [Badewanne3](https://osu.ppy.sh/u/2211396) all around osu!"
    );

    let join_server = EmbedField {
        inline: false,
        name: "Got a question, suggestion, bug, or are interested in the development?".to_owned(),
        value: format!("Feel free to join the [discord server]({BATHBOT_WORKSHOP})"),
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
        value: with_comma_int(ctx.cache.stats().guilds()).to_string(),
    };

    let boot_time = ctx.stats.start_time;

    let boot_up = EmbedField {
        inline: true,
        name: "Boot-up".to_owned(),
        value: how_long_ago_dynamic(&boot_time).to_string(),
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
        value: with_comma_int(commands_used).to_string(),
    };

    let osu_requests: usize = ctx.stats.osu_metrics.rosu.collect()[0]
        .get_metric()
        .iter()
        .map(|metric| metric.get_counter().get_value() as usize)
        .sum();

    let osu_requests = EmbedField {
        inline: true,
        name: "osu!api requests".to_owned(),
        value: with_comma_int(osu_requests).to_string(),
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

    let builder = EmbedBuilder::new()
        .description(description)
        .fields(fields)
        .build()
        .into();

    command.callback(&ctx, builder, true).await?;

    Ok(())
}

async fn help_slash_command(
    ctx: &Context,
    command: Box<ApplicationCommand>,
    cmd: &SlashCommand,
) -> BotResult<()> {
    let ApplicationCommandData {
        name,
        description,
        help,
        options,
        ..
    } = (cmd.create)();

    let description = help.unwrap_or(description);

    if name == "owner" {
        let description =
            "This command can only be used by the owner of the bot.\nQuit snooping around :^)";

        let embed_builder = EmbedBuilder::new().title(name).description(description);
        let builder = MessageBuilder::new().embed(embed_builder);
        command.callback(ctx, builder, true).await?;

        return Ok(());
    }

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
