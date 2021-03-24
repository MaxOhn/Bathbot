use crate::embeds::{EmbedData, EmbedFields};

use std::{collections::HashSet, fmt::Write};

pub struct UntrackEmbed {
    title: &'static str,
    fields: EmbedFields,
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

            fields.push(("No longer tracking:".to_owned(), value, false));
        }

        if let Some(failed) = failed {
            fields.push((
                "Failed to untrack:".to_owned(),
                format!("`{}`", failed),
                false,
            ));
        }

        Self { title, fields }
    }
}

impl EmbedData for UntrackEmbed {
    fn title_owned(&mut self) -> Option<String> {
        Some(self.title.to_owned())
    }
    fn fields_owned(self) -> Option<EmbedFields> {
        Some(self.fields)
    }
}
