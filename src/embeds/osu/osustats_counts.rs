use std::fmt::Write;

use command_macros::EmbedData;
use rosu_v2::prelude::GameMode;

use crate::{
    manager::redis::{osu::User, RedisData},
    util::{
        builder::AuthorBuilder,
        osu::{TopCount, TopCounts},
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct OsuStatsCountsEmbed {
    description: String,
    thumbnail: String,
    title: String,
    author: AuthorBuilder,
}

impl OsuStatsCountsEmbed {
    pub fn new(user: &RedisData<User>, mode: GameMode, counts: TopCounts) -> Self {
        let count_len = counts.count_len();

        let mut description = String::with_capacity(64);
        description.push_str("```\n");

        let has_top100 = counts.top100s.is_some();
        let top_n_len = 2 + has_top100 as usize;

        for TopCount { top_n, count, rank } in counts {
            let _ = write!(description, "Top {top_n:<top_n_len$}:  {count:>count_len$}");

            if let Some(rank) = rank {
                let _ = writeln!(description, "   #{rank}");
            } else {
                description.push('\n');
            }
        }

        description.push_str("```");

        let mode = match mode {
            GameMode::Osu => "",
            GameMode::Mania => "mania ",
            GameMode::Taiko => "taiko ",
            GameMode::Catch => "ctb ",
        };

        Self {
            description,
            author: user.author_builder(),
            thumbnail: user.avatar_url().to_owned(),
            title: format!(
                "In how many top X {mode}map leaderboards is {}?",
                user.username().cow_escape_markdown()
            ),
        }
    }
}
