use crate::{
    custom_client::OsuStatsPlayer,
    embeds::{Author, EmbedData, Footer},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        numbers::with_comma_u64,
    },
};

use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct OsuStatsListEmbed {
    description: String,
    thumbnail: ImageSource,
    author: Author,
    footer: Footer,
}

impl OsuStatsListEmbed {
    pub fn new(
        players: &[OsuStatsPlayer],
        country: &Option<String>,
        first_place_id: u32,
        pages: (usize, usize),
    ) -> Self {
        let mut author = Author::new("Most global leaderboard scores");
        if let Some(country) = country {
            author = author.icon_url(format!("{}/images/flags/{}.png", OSU_BASE, country))
        }
        let mut description = String::with_capacity(1024);
        for (i, player) in players.iter().enumerate() {
            let _ = writeln!(
                description,
                "**{}. [{}]({}users/{})**: {}",
                (pages.0 - 1) * 15 + i + 1,
                player.username,
                OSU_BASE,
                player.user_id,
                with_comma_u64(player.count as u64)
            );
        }
        let thumbnail = ImageSource::url(format!("{}{}", AVATAR_URL, first_place_id)).unwrap();
        Self {
            author,
            thumbnail,
            description,
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
        }
    }
}

impl EmbedData for OsuStatsListEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
}
