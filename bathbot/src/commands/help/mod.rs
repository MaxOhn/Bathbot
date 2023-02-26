use std::{collections::BTreeMap, fmt::Write};

use twilight_interactions::command::CommandOptionExt;
use twilight_model::{
    application::command::CommandOptionType,
    channel::message::{
        component::{ActionRow, SelectMenu, SelectMenuOption},
        embed::EmbedField,
        Component,
    },
};

pub use self::{
    components::handle_help_component,
    interaction::{slash_help, Help, HELP_SLASH},
    message::{handle_help_category, HELP_PREFIX},
};

mod components;
mod interaction;
mod message;

const AUTHORITY_STATUS: &str = "Requires authority status (check the /authorities command)";

fn failed_message_content(dists: BTreeMap<usize, &'static str>) -> String {
    let mut names = dists.iter().take(5).map(|(_, &name)| name);

    if let Some(name) = names.next() {
        let count = dists.len().min(5);
        let mut content = String::with_capacity(14 + count * (5 + 2) + (count - 1) * 2);
        content.push_str("Did you mean ");
        let _ = write!(content, "`{name}`");

        for name in names {
            let _ = write!(content, ", `{name}`");
        }

        content.push('?');

        content
    } else {
        "There is no such command".to_owned()
    }
}

fn parse_select_menu(options: &[CommandOptionExt]) -> Option<Vec<Component>> {
    if options.is_empty() {
        return None;
    }

    let options: Vec<_> = options
        .iter()
        .filter_map(|option| match option.inner.kind {
            CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup => {
                Some((&option.inner.name, &option.inner.description))
            }
            _ => None,
        })
        .map(|(name, description)| SelectMenuOption {
            default: false,
            description: Some(description.to_owned()),
            emoji: None,
            label: name.to_owned(),
            value: name.to_owned(),
        })
        .collect();

    if options.is_empty() {
        return None;
    }

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

    Some(vec![Component::ActionRow(row)])
}

fn option_fields(children: &[CommandOptionExt]) -> Vec<EmbedField> {
    children
        .iter()
        .filter_map(|child| {
            if matches!(
                child.inner.kind,
                CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
            ) {
                return None;
            }

            let mut name = child.inner.name.clone();

            if child.inner.required.unwrap_or(false) {
                name.push_str(" (required)");
            }

            let value = child
                .help
                .as_ref()
                .map_or_else(|| child.inner.description.clone(), |help| help.to_owned());

            let field = EmbedField {
                inline: value.len() <= 40,
                name,
                value,
            };

            Some(field)
        })
        .collect()
}
