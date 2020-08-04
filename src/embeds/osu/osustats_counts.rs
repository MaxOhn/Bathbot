use crate::{
    embeds::{osu, Author, EmbedData},
    util::constants::AVATAR_URL,
};

use rosu::models::User;
use std::{collections::BTreeMap, fmt::Write};

#[derive(Clone)]
pub struct OsuStatsCountsEmbed {
    description: String,
    thumbnail: String,
    title: String,
    author: Author,
}

impl OsuStatsCountsEmbed {
    pub fn new(user: User, counts: BTreeMap<usize, String>) -> Self {
        let count_len = counts
            .iter()
            .fold(0, |max, (_, count)| max.max(count.len()));
        let mut description = String::with_capacity(64);
        description.push_str("```\n");
        for (rank, count) in counts {
            let _ = writeln!(
                description,
                "Top {:<2}: {:>count_len$}",
                rank,
                count,
                count_len = count_len,
            );
        }
        description.push_str("```");
        Self {
            description,
            author: osu::get_user_author(&user),
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
            title: format!("In how many top X map leaderboards is {}?", user.username),
        }
    }
}

impl EmbedData for OsuStatsCountsEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn thumbnail(&self) -> Option<&str> {
        Some(&self.thumbnail)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
}
