use crate::embeds::EmbedFields;

use std::{collections::HashSet, fmt::Write};

pub struct UntrackEmbed {
    fields: EmbedFields,
    title: &'static str,
}

impl UntrackEmbed {
    pub fn new(success: HashSet<String>, failed: Option<&String>) -> Self {
        let title = "Top score tracking";
        let mut fields = EmbedFields::new();
        let mut iter = success.iter();

        if let Some(first) = iter.next() {
            let names_len: usize = success.iter().map(|name| name.len() + 4).sum();
            let mut value = String::with_capacity(names_len);
            let _ = write!(value, "`{}`", first);

            for name in iter {
                let _ = write!(value, ", `{}`", name);
            }

            fields.push(field!("No longer tracking:", value, false));
        }

        if let Some(failed) = failed {
            fields.push(field!("Failed to untrack:", format!("`{}`", failed), false));
        }

        Self { fields, title }
    }
}

impl_into_builder!(UntrackEmbed { fields, title });
