use bathbot_util::{EmbedBuilder, FooterBuilder};
use eyre::Result;
use futures::future::{ready, BoxFuture};
use twilight_interactions::command::{ApplicationCommandData, CommandOptionExt};
use twilight_model::{
    application::command::CommandOptionType,
    channel::message::{
        component::{ActionRow, Button, ButtonStyle, SelectMenu, SelectMenuOption},
        embed::EmbedField,
        Component,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    core::commands::interaction::{
        twilight_command::Command, InteractionCommandKind, InteractionCommands,
    },
    util::{interaction::InteractionComponent, Authored},
};

const AUTHORITY_STATUS: &str = "Requires authority status (check the /authorities command)";

pub struct HelpInteractionCommand {
    next_title: String,
    msg_owner: Id<UserMarker>,
}

impl IActiveMessage for HelpInteractionCommand {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let Some(command) = self.find_command() else {
            let err = eyre!("Unknown command title={:?}", self.next_title);

            return Box::pin(ready(Err(err)));
        };

        let parts = match self.command_parts(command) {
            Ok(parts) => parts,
            Err(err) => return Box::pin(ready(Err(err))),
        };

        let CommandParts {
            help,
            root: _,
            options,
        } = parts;

        let mut embed = EmbedBuilder::new()
            .title(self.next_title.clone())
            .description(help)
            .fields(option_fields(options));

        if command.flags().authority() {
            embed = embed.footer(FooterBuilder::new(AUTHORITY_STATUS));
        }

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        let Some(command) = self.find_command() else {
            warn!(title = self.next_title, "Unknown command");

            return Vec::new();
        };

        let parts = match self.command_parts(command) {
            Ok(parts) => parts,
            Err(err) => {
                warn!(?err, "Failed to get command parts");

                return Vec::new();
            }
        };

        let CommandParts {
            help: _,
            root,
            options,
        } = parts;

        if root && options.is_empty() {
            return Vec::new();
        }

        let options: Vec<_> = options
            .into_iter()
            .filter_map(|option| match option.inner.kind {
                CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup => {
                    Some((option.inner.name, option.inner.description))
                }
                _ => None,
            })
            .map(|(name, description)| SelectMenuOption {
                default: false,
                description: Some(description),
                emoji: None,
                label: name.clone(),
                value: name,
            })
            .collect();

        let mut components = Vec::with_capacity(2);

        if !options.is_empty() {
            let select_menu = SelectMenu {
                custom_id: "help_menu".to_owned(),
                disabled: false,
                max_values: None,
                min_values: None,
                options,
                placeholder: Some("Select a subcommand".to_owned()),
            };

            let row = ActionRow {
                components: vec![Component::SelectMenu(select_menu)],
            };

            components.push(Component::ActionRow(row));
        }

        let back_button = Button {
            custom_id: Some("help_back".to_owned()),
            disabled: root,
            emoji: None,
            label: Some("Back".to_owned()),
            style: ButtonStyle::Danger,
            url: None,
        };

        let button_row = ActionRow {
            components: vec![Component::Button(back_button)],
        };

        components.push(Component::ActionRow(button_row));

        components
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err).boxed(),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore.boxed();
        }

        match component.data.custom_id.as_str() {
            "help_menu" => self.handle_menu(component).boxed(),
            "help_back" => self.handle_back().boxed(),
            other => {
                warn!(name = %other, ?component, "Unknown interaction help component");

                ComponentResult::Ignore.boxed()
            }
        }
    }
}

impl HelpInteractionCommand {
    pub fn new(command: String, msg_owner: Id<UserMarker>) -> Self {
        Self {
            next_title: command,
            msg_owner,
        }
    }

    fn find_command(&self) -> Option<InteractionCommandKind> {
        let base = self.next_title.split(' ').next()?;

        InteractionCommands::get().command(base)
    }

    fn command_parts(&self, command: InteractionCommandKind) -> Result<CommandParts> {
        let mut iter = CommandIter::from(command);

        if let CommandIterStatus::DoneOrInvalidName = iter.parse(&self.next_title) {
            let err = eyre!("CommandIter failed to parse title `{}`", self.next_title);

            return Err(err);
        }

        Ok(iter.into_parts())
    }

    fn handle_menu(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        let Some(name) = component.data.values.pop() else {
            return ComponentResult::Err(eyre!("Missing value in interaction help menu"));
        };

        self.next_title.push(' ');
        self.next_title.push_str(&name);

        ComponentResult::BuildPage
    }

    fn handle_back(&mut self) -> ComponentResult {
        let Some(split_idx) = self.next_title.rfind(' ') else {
            return ComponentResult::Err(eyre!("Missing whitespace in interaction help title"));
        };

        self.next_title.truncate(split_idx);

        ComponentResult::BuildPage
    }
}

fn option_fields(children: Vec<CommandOptionExt>) -> Vec<EmbedField> {
    children
        .into_iter()
        .filter_map(|child| {
            if matches!(
                child.inner.kind,
                CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
            ) {
                return None;
            }

            let mut name = child.inner.name;

            if child.inner.required.unwrap_or(false) {
                name.push_str(" (required)");
            }

            let value = child.help.unwrap_or(child.inner.description);

            let field = EmbedField {
                inline: value.len() <= 40,
                name,
                value,
            };

            Some(field)
        })
        .collect()
}

enum EitherCommand {
    Base {
        command: InteractionCommandKind,
        used: bool,
    },
    Option(Box<CommandOptionExt>),
}

struct CommandIter {
    curr: EitherCommand,
    next: Option<CommandOptionExt>,
}

enum CommandIterStatus {
    Match,
    DoneOrInvalidName,
}

impl From<InteractionCommandKind> for CommandIter {
    #[inline]
    fn from(command: InteractionCommandKind) -> Self {
        Self {
            curr: EitherCommand::Base {
                command,
                used: false,
            },
            next: None,
        }
    }
}

impl CommandIter {
    fn parse(&mut self, commands: &str) -> CommandIterStatus {
        for name in commands.split(' ').filter(|name| !name.is_empty()) {
            if let CommandIterStatus::DoneOrInvalidName = self.next(name) {
                return CommandIterStatus::DoneOrInvalidName;
            }
        }

        CommandIterStatus::Match
    }

    fn next(&mut self, name: &str) -> CommandIterStatus {
        let options = match &mut self.next {
            Some(option) => {
                if matches!(
                    option.inner.kind,
                    CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
                ) {
                    option.inner.options.take().unwrap_or_default()
                } else {
                    return CommandIterStatus::DoneOrInvalidName;
                }
            }
            None => match &mut self.curr {
                EitherCommand::Base { command, used } => {
                    let (name_, options) = match command {
                        InteractionCommandKind::Chat(command) => {
                            let ApplicationCommandData {
                                name: name_,
                                options,
                                ..
                            } = (command.create)();

                            (name_, options)
                        }
                        InteractionCommandKind::Message(command) => {
                            let Command { name: name_, .. } = (command.create)();

                            (name_, Vec::new())
                        }
                    };

                    if *used {
                        options
                    } else if name != name_ {
                        return CommandIterStatus::DoneOrInvalidName;
                    } else {
                        *used = true;

                        return CommandIterStatus::Match;
                    }
                }
                EitherCommand::Option(option) => {
                    if matches!(
                        option.inner.kind,
                        CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
                    ) {
                        option.inner.options.take().unwrap_or_default()
                    } else {
                        return CommandIterStatus::DoneOrInvalidName;
                    }
                }
            },
        };

        let Some(next) = options.into_iter().find(|o| o.inner.name == name) else {
            return CommandIterStatus::DoneOrInvalidName;
        };

        if let Some(curr) = self.next.replace(next) {
            self.curr = EitherCommand::Option(Box::new(curr));
        }

        CommandIterStatus::Match
    }

    fn into_parts(self) -> CommandParts {
        CommandParts::from(self)
    }
}

struct CommandParts {
    help: String,
    root: bool,
    options: Vec<CommandOptionExt>,
}

impl From<InteractionCommandKind> for CommandParts {
    #[inline]
    fn from(command: InteractionCommandKind) -> Self {
        match command {
            InteractionCommandKind::Chat(command) => {
                let ApplicationCommandData {
                    help,
                    description,
                    options,
                    ..
                } = (command.create)();

                Self {
                    help: help.unwrap_or(description),
                    root: true,
                    options,
                }
            }
            InteractionCommandKind::Message(command) => {
                let Command { description, .. } = (command.create)();

                Self {
                    help: description,
                    root: true,
                    options: Vec::new(),
                }
            }
        }
    }
}

impl From<CommandOptionExt> for CommandParts {
    #[inline]
    fn from(option: CommandOptionExt) -> Self {
        Self {
            help: option.help.unwrap_or(option.inner.description),
            root: false,
            options: option.inner.options.unwrap_or_default(),
        }
    }
}

impl From<EitherCommand> for CommandParts {
    #[inline]
    fn from(either: EitherCommand) -> Self {
        match either {
            EitherCommand::Base { command, .. } => command.into(),
            EitherCommand::Option(option) => (*option).into(),
        }
    }
}

impl From<CommandIter> for CommandParts {
    #[inline]
    fn from(iter: CommandIter) -> Self {
        match iter.next {
            Some(option) => option.into(),
            None => iter.curr.into(),
        }
    }
}
