use crate::{
    custom_client::OsekaiRarityEntry,
    embeds::Footer,
    util::{numbers::round, CowUtils},
};

use std::fmt::Write;

pub struct MedalRarityEmbed {
    title: &'static str,
    description: String,
    footer: Footer,
}

impl MedalRarityEmbed {
    pub fn new(ranking: &[OsekaiRarityEntry], index: usize) -> Self {
        let mut description = String::with_capacity(1024);

        for (i, entry) in ranking.iter().enumerate() {
            let url_name = entry
                .medal_name
                .cow_replace(' ', "+")
                .cow_replace(',', "%2C");

            let _ = writeln!(
                description,
                "**{idx}. [{medal}]({url})**: `{rarity}%`\n ▶ `{description}`",
                idx = index + i + 1,
                medal = entry.medal_name,
                url = url_name,
                rarity = round(entry.possession_percent),
                description = entry.description,
            );
        }

        Self {
            description,
            footer: Footer::new("Check out osekai.net for more info"),
            title: "Medal rarity leaderboard",
        }
    }
}

impl_builder!(MedalRarityEmbed {
    description,
    footer,
    title,
});
