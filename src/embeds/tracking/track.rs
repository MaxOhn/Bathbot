use crate::embeds::EmbedFields;

use rosu_v2::model::GameMode;
use std::fmt::Write;

pub struct TrackEmbed {
    fields: EmbedFields,
    title: String,
}

impl TrackEmbed {
    pub fn new(
        mode: GameMode,
        success: Vec<String>,
        failure: Vec<String>,
        failed: Option<String>,
        limit: usize,
    ) -> Self {
        let title = format!("Top score tracking | mode={} | limit={}", mode, limit);
        let mut fields = EmbedFields::new();
        let mut iter = success.iter();

        if let Some(first) = iter.next() {
            let names_len: usize = success.iter().map(|name| name.len() + 4).sum();
            let mut value = String::with_capacity(names_len);
            let _ = write!(value, "`{}`", first);

            for name in iter {
                let _ = write!(value, ", `{}`", name);
            }

            fields.push(field!("Now tracking:".to_owned(), value, false));
        }

        let mut iter = failure.iter();

        if let Some(first) = iter.next() {
            let names_len: usize = success.iter().map(|name| name.len() + 4).sum();
            let mut value = String::with_capacity(names_len);
            let _ = write!(value, "`{}`", first);

            for name in iter {
                let _ = write!(value, ", `{}`", name);
            }

            fields.push(field!("Already tracked:".to_owned(), value, false));
        }

        if let Some(failed) = failed {
            fields.push(field!(
                "Failed to track:".to_owned(),
                format!("`{}`", failed),
                false
            ));
        }

        Self { fields, title }
    }
}

impl_builder!(TrackEmbed { fields, title });
