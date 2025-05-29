use std::borrow::Cow;

use bathbot_util::{Authored, EmbedBuilder, FooterBuilder};
use eyre::Result;
use twilight_interactions::command::{ApplicationCommandData, CommandOptionExtended};
use twilight_model::{
    application::command::{Command, CommandOptionType},
    channel::message::{
        Component,
        component::{ActionRow, Button, ButtonStyle, SelectMenu, SelectMenuOption, SelectMenuType},
        embed::EmbedField,
    },
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    core::commands::interaction::{InteractionCommandKind, InteractionCommands},
    util::interaction::InteractionComponent,
};

const AUTHORITY_STATUS: &str =
    "Requires authority status (check the `/serverconfig authorities` command)";

pub struct HelpInteractionCommand {
    next_title: String,
    msg_owner: Id<UserMarker>,
}

impl IActiveMessage for HelpInteractionCommand {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let Some(command) = self.find_command() else {
            bail!("Unknown command title={:?}", self.next_title);
        };

        let parts = match self.command_parts(command) {
            Ok(parts) => parts,
            Err(err) => return Err(err),
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

        Ok(BuildPage::new(embed, false))
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
            .filter_map(|option| match option.kind {
                CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup => {
                    Some((option.name, option.description))
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
                options: Some(options),
                placeholder: Some("Select a subcommand".to_owned()),
                channel_types: None,
                default_values: None,
                kind: SelectMenuType::Text,
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
            sku_id: None,
        };

        let button_row = ActionRow {
            components: vec![Component::Button(back_button)],
        };

        components.push(Component::ActionRow(button_row));

        components
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore;
        }

        match component.data.custom_id.as_str() {
            "help_menu" => self.handle_menu(component),
            "help_back" => self.handle_back(),
            other => {
                warn!(name = %other, ?component, "Unknown interaction help component");

                ComponentResult::Ignore
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

fn option_fields(children: Vec<CommandOptionExtended>) -> Vec<EmbedField> {
    children
        .into_iter()
        .filter_map(|child| {
            if matches!(
                child.kind,
                CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
            ) {
                return None;
            }

            let mut name = child.name;

            if child.required.unwrap_or(false) {
                name.push_str(" (required)");
            }

            let value = child
                .help
                .map(ToString::to_string)
                .unwrap_or(child.description);

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
    Option(Box<CommandOptionExtended>),
}

struct CommandIter {
    curr: EitherCommand,
    next: Option<CommandOptionExtended>,
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
                    option.kind,
                    CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
                ) {
                    option.options.take().unwrap_or_default()
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
                        option.kind,
                        CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
                    ) {
                        option.options.take().unwrap_or_default()
                    } else {
                        return CommandIterStatus::DoneOrInvalidName;
                    }
                }
            },
        };

        let Some(next) = options.into_iter().find(|o| o.name == name) else {
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
    help: Cow<'static, str>,
    root: bool,
    options: Vec<CommandOptionExtended>,
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
                    help: help.map_or(Cow::Owned(description), Cow::Borrowed),
                    root: true,
                    options,
                }
            }
            InteractionCommandKind::Message(command) => {
                let Command { description, .. } = (command.create)();

                Self {
                    help: Cow::Owned(description),
                    root: true,
                    options: Vec::new(),
                }
            }
        }
    }
}

impl From<CommandOptionExtended> for CommandParts {
    #[inline]
    fn from(option: CommandOptionExtended) -> Self {
        Self {
            help: option
                .help
                .map_or(Cow::Owned(option.description), Cow::Borrowed),
            root: false,
            options: option.options.unwrap_or_default(),
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
