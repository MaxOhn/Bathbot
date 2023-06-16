use std::fmt::Write;

use bathbot_model::rosu_v2::user::User;
use bathbot_util::{AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;

use crate::{
    embeds::EmbedData,
    manager::redis::RedisData,
    util::osu::{TopCount, TopCounts},
};

pub struct OsuStatsCountsEmbed {
    description: String,
    thumbnail: String,
    title: String,
    author: AuthorBuilder,
    footer_timestamp: Option<(FooterBuilder, OffsetDateTime)>,
}

impl OsuStatsCountsEmbed {
    pub fn new(user: &RedisData<User>, mode: GameMode, counts: TopCounts) -> Self {
        let count_len = counts.count_len();

        let footer_timestamp = counts
            .last_update
            .map(|datetime| (FooterBuilder::new("Last Update"), datetime));

        let mut description = String::with_capacity(64);
        description.push_str("```\n");

        for TopCount { top_n, count, rank } in counts {
            let _ = write!(description, "Top {top_n:<3}:  {count:>count_len$}");

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
            footer_timestamp,
            title: format!(
                "In how many top X {mode}map leaderboards is {}?",
                user.username().cow_escape_markdown()
            ),
        }
    }
}

impl EmbedData for OsuStatsCountsEmbed {
    #[inline]
    fn build(self) -> EmbedBuilder {
        let mut builder = EmbedBuilder::new()
            .description(self.description)
            .title(self.title)
            .thumbnail(self.thumbnail)
            .author(self.author);

        if let Some((footer, timestamp)) = self.footer_timestamp {
            builder = builder.footer(footer).timestamp(timestamp);
        }

        builder
    }
}
