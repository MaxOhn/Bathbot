use crate::{embeds::Author, util::constants::AVATAR_URL};

use rosu_v2::prelude::{GameMode, User};
use std::{borrow::Cow, collections::BTreeMap, fmt::Write};

pub struct OsuStatsCountsEmbed {
    description: String,
    thumbnail: String,
    title: String,
    author: Author,
}

impl OsuStatsCountsEmbed {
    pub fn new(user: User, mode: GameMode, counts: BTreeMap<usize, Cow<'static, str>>) -> Self {
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

        let mode = match mode {
            GameMode::STD => "",
            GameMode::MNA => "mania ",
            GameMode::TKO => "taiko ",
            GameMode::CTB => "ctb ",
        };

        description.push_str("```");

        Self {
            description,
            author: author!(user),
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
            title: format!(
                "In how many top X {}map leaderboards is {}?",
                mode, user.username
            ),
        }
    }
}

impl_into_builder!(OsuStatsCountsEmbed {
    author,
    description,
    thumbnail,
    title,
});
