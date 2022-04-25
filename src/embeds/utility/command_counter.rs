
use std::fmt::Write;

use chrono::{DateTime, Utc};
use command_macros::EmbedData;

use crate::util::{
    builder::{AuthorBuilder, FooterBuilder},
    constants::SYMBOLS,
    datetime::how_long_ago_text,
};

#[derive(EmbedData)]
pub struct CommandCounterEmbed {
    description: String,
    footer: FooterBuilder,
    author: AuthorBuilder,
}

impl CommandCounterEmbed {
    pub fn new(
        list: Vec<(&String, u32)>,
        booted_up: &DateTime<Utc>,
        idx: usize,
        pages: (usize, usize),
    ) -> Self {
        let len = list
            .iter()
            .fold(0, |max, (name, _)| max.max(name.chars().count()));

        let mut description = String::with_capacity(256);
        description.push_str("```\n");

        for (mut i, (name, amount)) in list.into_iter().enumerate() {
            i += idx;

            let _ = writeln!(
                description,
                "{i:>2} {:1} # {name:<len$} => {amount}",
                if i <= SYMBOLS.len() {
                    SYMBOLS[i - 1]
                } else {
                    ""
                },
                len = len
            );
        }

        description.push_str("```");

        let footer_text = format!(
            "Page {}/{} ~ Started counting {}",
            pages.0,
            pages.1,
            how_long_ago_text(booted_up)
        );

        Self {
            description,
            footer: FooterBuilder::new(footer_text),
            author: AuthorBuilder::new("Most popular commands:"),
        }
    }
}
