use std::fmt::Write;

use bathbot_util::{EmbedBuilder, FooterBuilder, MessageBuilder};
use eyre::{ContextCompat, Result};
use twilight_interactions::command::CommandOptionExt;
use twilight_model::{
    application::command::CommandOptionType,
    channel::message::{
        component::{ActionRow, Button, ButtonStyle},
        Component,
    },
};

use super::{option_fields, parse_select_menu, AUTHORITY_STATUS};
use crate::{
    core::{
        commands::slash::{SlashCommand, SlashCommands},
        Context,
    },
    util::{interaction::InteractionComponent, ComponentExt},
};

type PartResult = Result<(Parts, bool)>;

struct Parts {
    name: String,
    help: String,
    root: bool,
    options: Vec<CommandOptionExt>,
}

impl From<&'static SlashCommand> for Parts {
    #[inline]
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
    #[inline]
    fn from(option: CommandOptionExt) -> Self {
        Self {
            name: option.inner.name,
            help: option.help.unwrap_or(option.inner.description),
            root: false,
            options: option.inner.options.unwrap_or_default(),
        }
    }
}

impl From<EitherCommand> for Parts {
    #[inline]
    fn from(either: EitherCommand) -> Self {
        match either {
            EitherCommand::Base(command) => command.into(),
            EitherCommand::Option(option) => (*option).into(),
        }
    }
}

impl From<CommandIter> for Parts {
    #[inline]
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
    #[inline]
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
            Some(option) => {
                if matches!(
                    option.inner.kind,
                    CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
                ) {
                    option.inner.options.take().unwrap_or_default()
                } else {
                    return true;
                }
            }
            None => match &mut self.curr {
                EitherCommand::Base(command) => (command.create)().options,
                EitherCommand::Option(option) => {
                    if matches!(
                        option.inner.kind,
                        CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
                    ) {
                        option.inner.options.take().unwrap_or_default()
                    } else {
                        return true;
                    }
                }
            },
        };

        let next = match options.into_iter().find(|o| o.inner.name == name) {
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
) -> Result<()> {
    let mut title = component
        .message
        .embeds
        .pop()
        .wrap_err("missing embed")?
        .title
        .wrap_err("missing embed title")?;

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
    let base = names.next().wrap_err("missing embed title")?;

    let command = SlashCommands::get()
        .command(base)
        .wrap_err("unknown command")?;

    let authority = command.flags.authority();
    let mut iter = CommandIter::from(command);

    for name in names {
        if iter.next(name) {
            bail!("unknown command");
        }
    }

    if iter.next(name) {
        bail!("unknown command");
    }

    let command = Parts::from(iter);
    let _ = write!(title, " {}", command.name);

    Ok((command, authority))
}

fn backtrack_subcommand(title: &mut String) -> PartResult {
    let index = title.chars().filter(char::is_ascii_whitespace).count();
    let mut names = title.split(' ').take(index);
    let base = names.next().wrap_err("missing embed title")?;

    let command = SlashCommands::get()
        .command(base)
        .wrap_err("unknown command")?;

    let authority = command.flags.authority();

    let mut iter = CommandIter::from(command);

    for name in names {
        if iter.next(name) {
            bail!("unknown command");
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
