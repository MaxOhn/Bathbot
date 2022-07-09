use std::{borrow::Cow, collections::BTreeMap, fmt::Write};

use command_macros::EmbedData;
use rosu_v2::prelude::{GameMode, User};

use crate::util::{builder::AuthorBuilder, CowUtils};

#[derive(EmbedData)]
pub struct OsuStatsCountsEmbed {
    description: String,
    thumbnail: String,
    title: String,
    author: AuthorBuilder,
}

impl OsuStatsCountsEmbed {
    pub fn new(user: User, mode: GameMode, counts: BTreeMap<usize, Cow<'static, str>>) -> Self {
        let count_len = counts
            .iter()
            .fold(0, |max, (_, count)| max.max(count.len()));

        let mut description = String::with_capacity(64);
        description.push_str("```\n");

        for (rank, count) in counts {
            let _ = writeln!(description, "Top {rank:<2}: {count:>count_len$}",);
        }

        let mode = match mode {
            GameMode::Osu => "",
            GameMode::Mania => "mania ",
            GameMode::Taiko => "taiko ",
            GameMode::Catch => "ctb ",
        };

        description.push_str("```");

        Self {
            description,
            author: author!(user),
            thumbnail: user.avatar_url,
            title: format!(
                "In how many top X {mode}map leaderboards is {}?",
                user.username.cow_escape_markdown()
            ),
        }
    }
}
