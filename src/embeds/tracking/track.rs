use crate::embeds::EmbedData;

use rosu::models::GameMode;
use std::fmt::Write;

pub struct TrackEmbed {
    title: String,
    fields: Vec<(String, String, bool)>,
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
        let mut fields = Vec::new();
        let mut iter = success.iter();
        if let Some(first) = iter.next() {
            let names_len: usize = success.iter().map(|name| name.len() + 4).sum();
            let mut value = String::with_capacity(names_len);
            let _ = write!(value, "`{}`", first);
            for name in iter {
                let _ = write!(value, ", `{}`", name);
            }
            fields.push(("Now tracking:".to_owned(), value, false));
        }
        let mut iter = failure.iter();
        if let Some(first) = iter.next() {
            let names_len: usize = success.iter().map(|name| name.len() + 4).sum();
            let mut value = String::with_capacity(names_len);
            let _ = write!(value, "`{}`", first);
            for name in iter {
                let _ = write!(value, ", `{}`", name);
            }
            fields.push(("Already tracked:".to_owned(), value, false));
        }
        if let Some(failed) = failed {
            fields.push((
                "Failed to track:".to_owned(),
                format!("`{}`", failed),
                false,
            ));
        }
        Self { title, fields }
    }
}

impl EmbedData for TrackEmbed {
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
}
