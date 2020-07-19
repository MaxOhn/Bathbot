use crate::{
    embeds::{Author, EmbedData, Footer},
    util::constants::SYMBOLS,
};

use std::fmt::Write;

#[derive(Clone)]
pub struct BGRankingEmbed {
    author: Author,
    description: String,
    footer: Footer,
}

impl BGRankingEmbed {
    pub fn new(
        author_idx: Option<usize>,
        list: Vec<(&String, u32)>,
        global: bool,
        idx: usize,
        pages: (usize, usize),
    ) -> Self {
        let len = list
            .iter()
            .fold(0, |max, (user, _)| max.max(user.chars().count()));
        let mut description = String::with_capacity(256);
        description.push_str("```\n");
        for (mut i, (user, score)) in list.into_iter().enumerate() {
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
                user,
                score,
                len = len
            );
        }
        description.push_str("```");
        let mut footer_text = format!("Page {}/{}", pages.0, pages.1);
        if let Some(author_idx) = author_idx {
            let _ = write!(footer_text, " ~ Your rank: {}", author_idx + 1);
        }
        let author_text = format!(
            "{} leaderboard for correct guesses:",
            if global { "Global" } else { "Server" }
        );
        Self {
            author: Author::new(author_text),
            description,
            footer: Footer::new(footer_text),
        }
    }
}

impl EmbedData for BGRankingEmbed {
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
}
