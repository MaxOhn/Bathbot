use std::{fmt::Write, mem};

use twilight_interactions::command::{CommandOptionExt, CommandOptionExtInner};
use twilight_model::application::component::{button::ButtonStyle, ActionRow, Button, Component};

use crate::{
    core::{
        commands::slash::{SlashCommand, SlashCommands},
        Context,
    },
    error::InvalidHelpState,
    util::{
        builder::{EmbedBuilder, FooterBuilder, MessageBuilder},
        interaction::InteractionComponent,
        ComponentExt,
    },
    BotResult,
};

use super::{option_fields, parse_select_menu, AUTHORITY_STATUS};

type PartResult = Result<(Parts, bool), InvalidHelpState>;

struct Parts {
    name: String,
    help: String,
    root: bool,
    options: Vec<CommandOptionExt>,
}

impl From<&'static SlashCommand> for Parts {
    fn from(command: &'static SlashCommand) -> Self {
        let command = (command.create)();

        Self {
            name: command.name,
            help: command.help.unwrap_or(command.description),
            root: true,
            options: command.options,
        }
    }
}

impl From<CommandOptionExt> for Parts {
    fn from(option: CommandOptionExt) -> Self {
        let (name, description, options) = match option.inner {
            CommandOptionExtInner::SubCommand(o) | CommandOptionExtInner::SubCommandGroup(o) => {
                (o.name, o.description, o.options)
            }
            CommandOptionExtInner::String(d) => (d.name, d.description, Vec::new()),
            CommandOptionExtInner::Integer(d) => (d.name, d.description, Vec::new()),
            CommandOptionExtInner::Number(d) => (d.name, d.description, Vec::new()),
            CommandOptionExtInner::Boolean(d) => (d.name, d.description, Vec::new()),
            CommandOptionExtInner::User(d) => (d.name, d.description, Vec::new()),
            CommandOptionExtInner::Channel(d) => (d.name, d.description, Vec::new()),
            CommandOptionExtInner::Role(d) => (d.name, d.description, Vec::new()),
            CommandOptionExtInner::Mentionable(d) => (d.name, d.description, Vec::new()),
            CommandOptionExtInner::Attachment(d) => (d.name, d.description, Vec::new()),
        };

        Self {
            name,
            help: option.help.unwrap_or(description),
            root: false,
            options,
        }
    }
}

impl From<EitherCommand> for Parts {
    fn from(either: EitherCommand) -> Self {
        match either {
            EitherCommand::Base(command) => command.into(),
            EitherCommand::Option(option) => (*option).into(),
        }
    }
}

impl From<CommandIter> for Parts {
    fn from(iter: CommandIter) -> Self {
        match iter.next {
            Some(option) => option.into(),
            None => iter.curr.into(),
        }
    }
}

enum EitherCommand {
    Base(&'static SlashCommand),
    Option(Box<CommandOptionExt>),
}

struct CommandIter {
    curr: EitherCommand,
    next: Option<CommandOptionExt>,
}

impl From<&'static SlashCommand> for CommandIter {
    fn from(command: &'static SlashCommand) -> Self {
        Self {
            curr: EitherCommand::Base(command),
            next: None,
        }
    }
}

impl CommandIter {
    fn next(&mut self, name: &str) -> bool {
        let options = match &mut self.next {
            Some(option) => match &mut option.inner {
                CommandOptionExtInner::SubCommand(o)
                | CommandOptionExtInner::SubCommandGroup(o) => mem::take(&mut o.options),
                _ => return true,
            },
            None => match &mut self.curr {
                EitherCommand::Base(command) => (command.create)().options,
                EitherCommand::Option(option) => match &mut option.inner {
                    CommandOptionExtInner::SubCommand(o)
                    | CommandOptionExtInner::SubCommandGroup(o) => mem::take(&mut o.options),
                    _ => return true,
                },
            },
        };

        let next = match options.into_iter().find(|o| o.inner.name() == name) {
            Some(option) => option,
            None => return true,
        };

        if let Some(curr) = self.next.replace(next) {
            self.curr = EitherCommand::Option(Box::new(curr));
        }

        false
    }
}

pub async fn handle_help_component(
    ctx: &Context,
    mut component: InteractionComponent,
) -> BotResult<()> {
    let mut title = component
        .message
        .embeds
        .pop()
        .ok_or(InvalidHelpState::MissingEmbed)?
        .title
        .ok_or(InvalidHelpState::MissingTitle)?;

    // If value is None, back button was pressed; otherwise subcommand was picked
    let (command, authority) = match component.data.values.pop() {
        Some(name) => continue_subcommand(&mut title, &name)?,
        None => backtrack_subcommand(&mut title)?,
    };

    // Prepare embed and components
    let mut embed_builder = EmbedBuilder::new()
        .title(title)
        .description(command.help)
        .fields(option_fields(&command.options));

    if authority {
        embed_builder = embed_builder.footer(FooterBuilder::new(AUTHORITY_STATUS));
    }

    let mut components =
        parse_select_menu(&command.options).unwrap_or_else(|| Vec::with_capacity(1));

    let button_row = ActionRow {
        components: vec![back_button(command.root)],
    };

    components.push(Component::ActionRow(button_row));

    let builder = MessageBuilder::new()
        .embed(embed_builder)
        .components(components);

    component.callback(ctx, builder).await?;

    Ok(())
}

fn continue_subcommand(title: &mut String, name: &str) -> PartResult {
    let mut names = title.split(' ');
    let base = names.next().ok_or(InvalidHelpState::MissingTitle)?;

    let command = SlashCommands::get()
        .command(base)
        .ok_or(InvalidHelpState::UnknownCommand)?;

    let authority = command.flags.authority();
    let mut iter = CommandIter::from(command);

    for name in names {
        if iter.next(name) {
            return Err(InvalidHelpState::UnknownCommand);
        }
    }

    if iter.next(name) {
        return Err(InvalidHelpState::UnknownCommand);
    }

    let command = Parts::from(iter);
    let _ = write!(title, " {}", command.name);

    Ok((command, authority))
}

fn backtrack_subcommand(title: &mut String) -> PartResult {
    let index = title.chars().filter(char::is_ascii_whitespace).count();
    let mut names = title.split(' ').take(index);
    let base = names.next().ok_or(InvalidHelpState::MissingTitle)?;

    let command = SlashCommands::get()
        .command(base)
        .ok_or(InvalidHelpState::UnknownCommand)?;

    let authority = command.flags.authority();

    let mut iter = CommandIter::from(command);

    for name in names {
        if iter.next(name) {
            return Err(InvalidHelpState::UnknownCommand);
        }
    }

    if let Some(pos) = title.rfind(' ') {
        title.truncate(pos);
    }

    Ok((iter.into(), authority))
}

fn back_button(disabled: bool) -> Component {
    let button = Button {
        custom_id: Some("help_back".to_owned()),
        disabled,
        emoji: None,
        label: Some("Back".to_owned()),
        style: ButtonStyle::Danger,
        url: None,
    };

    Component::Button(button)
}
