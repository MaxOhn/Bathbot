use crate::{
    embeds::{Author, EmbedData, Footer},
    util::globals::SYMBOLS,
};

use std::fmt::Write;

#[derive(Clone)]
pub struct CommandCounterEmbed {
    description: String,
    footer: Footer,
    author: Author,
}

impl CommandCounterEmbed {
    pub fn new(
        list: Vec<(&String, u32)>,
        booted_up: &str,
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
                "{:>2} {:1} # {:<len$} => {}",
                i,
                if i <= SYMBOLS.len() {
                    SYMBOLS[i - 1]
                } else {
                    ""
                },
                name,
                amount,
                len = len
            );
        }
        description.push_str("```");
        let footer_text = format!(
            "Page {}/{} ~ Started counting {}",
            pages.0, pages.1, booted_up
        );
        Self {
            description,
            footer: Footer::new(footer_text),
            author: Author::new("Most popular commands:".to_owned()),
        }
    }
}

impl EmbedData for CommandCounterEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
}
