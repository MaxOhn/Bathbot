use std::fmt::Write;

use command_macros::EmbedData;
use time::OffsetDateTime;

use crate::{
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::SYMBOLS,
        datetime::how_long_ago_text,
    },
};

#[derive(EmbedData)]
pub struct CommandCounterEmbed {
    description: String,
    footer: FooterBuilder,
    author: AuthorBuilder,
}

impl CommandCounterEmbed {
    pub fn new(list: Vec<(&String, u32)>, booted_up: &OffsetDateTime, pages: &Pages) -> Self {
        let len = list
            .iter()
            .fold(0, |max, (name, _)| max.max(name.chars().count()));

        let mut description = String::with_capacity(256);
        description.push_str("```\n");

        for ((name, amount), i) in list.into_iter().zip(pages.index + 1..) {
            let _ = writeln!(
                description,
                "{i:>2} {:1} # {name:<len$} => {amount}",
                SYMBOLS.get(i - 1).unwrap_or(&"")
            );
        }

        description.push_str("```");

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer_text = format!(
            "Page {page}/{pages} ~ Started counting {}",
            how_long_ago_text(booted_up)
        );

        Self {
            description,
            footer: FooterBuilder::new(footer_text),
            author: AuthorBuilder::new("Most popular commands:"),
        }
    }
}
