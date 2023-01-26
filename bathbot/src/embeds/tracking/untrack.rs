use std::{collections::HashSet, fmt::Write};

use bathbot_macros::EmbedData;
use rosu_v2::prelude::Username;
use twilight_model::channel::embed::EmbedField;

#[derive(EmbedData)]
pub struct UntrackEmbed {
    fields: Vec<EmbedField>,
    title: &'static str,
}

impl UntrackEmbed {
    pub fn new(success: HashSet<Username>, failed: Option<&Username>) -> Self {
        let title = "Top score tracking";
        let mut fields = Vec::with_capacity(2);
        let mut iter = success.iter();

        if let Some(first) = iter.next() {
            let names_len: usize = success.iter().map(|name| name.len() + 4).sum();
            let mut value = String::with_capacity(names_len);
            let _ = write!(value, "`{first}`");

            for name in iter {
                let _ = write!(value, ", `{name}`");
            }

            fields![fields { "No longer tracking:", value, false }];
        }

        if let Some(failed) = failed {
            fields![fields { "Failed to untrack:", format!("`{failed}`"), false }];
        }

        Self { fields, title }
    }
}
