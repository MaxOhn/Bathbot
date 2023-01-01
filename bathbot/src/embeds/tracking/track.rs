use std::fmt::Write;

use bathbot_macros::EmbedData;
use rosu_v2::{model::GameMode, prelude::Username};
use twilight_model::channel::embed::EmbedField;

#[derive(EmbedData)]
pub struct TrackEmbed {
    fields: Vec<EmbedField>,
    title: String,
}

impl TrackEmbed {
    pub fn new(
        mode: GameMode,
        success: Vec<Username>,
        failure: Vec<Username>,
        failed: Option<Username>,
        limit: u8,
    ) -> Self {
        let title = format!("Top score tracking | mode={} | limit={}", mode, limit);
        let mut fields = Vec::with_capacity(3);
        let mut iter = success.iter();

        if let Some(first) = iter.next() {
            let names_len: usize = success.iter().map(|name| name.len() + 4).sum();
            let mut value = String::with_capacity(names_len);
            let _ = write!(value, "`{}`", first);

            for name in iter {
                let _ = write!(value, ", `{}`", name);
            }

            fields![fields { "Now tracking:".to_owned(), value, false }];
        }

        let mut iter = failure.iter();

        if let Some(first) = iter.next() {
            let names_len: usize = success.iter().map(|name| name.len() + 4).sum();
            let mut value = String::with_capacity(names_len);
            let _ = write!(value, "`{}`", first);

            for name in iter {
                let _ = write!(value, ", `{}`", name);
            }

            fields![fields { "Already tracked:".to_owned(), value, false }];
        }

        if let Some(failed) = failed {
            fields![fields { "Failed to track:".to_owned(), format!("`{}`", failed), false }];
        }

        Self { fields, title }
    }
}
