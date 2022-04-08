use std::sync::Arc;

use command_macros::SlashCommand;
use twilight_interactions::{
    command::{ApplicationCommandData, CommandModel, CreateCommand},
    error::{ParseError, ParseOptionError, ParseOptionErrorType},
};
use twilight_model::application::{
    command::CommandOptionChoice,
    interaction::{ApplicationCommand, ApplicationCommandAutocomplete},
};

use crate::{
    core::{
        commands::slash::{SlashCommand, SLASH_COMMANDS},
        Context,
    },
    util::{
        builder::{EmbedBuilder, FooterBuilder, MessageBuilder},
        AutocompleteExt, CowUtils,
    },
    BotResult,
};

use super::{option_fields, parse_select_menu, AUTHORITY_STATUS};

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
                let builder = MessageBuilder::new().embed("failed help");
                command.callback(&ctx, builder, true).await?;

                Ok(())
            }
        },
        None => {
            let builder = MessageBuilder::new().embed("basic help");
            command.callback(&ctx, builder, true).await?;

            Ok(())
        }
    }
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
