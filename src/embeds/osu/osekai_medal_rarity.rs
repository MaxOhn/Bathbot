use std::fmt::Write;

use command_macros::EmbedData;

use crate::{
    custom_client::OsekaiRarityEntry,
    util::{builder::FooterBuilder, numbers::round, CowUtils},
};

#[derive(EmbedData)]
pub struct MedalRarityEmbed {
    description: String,
    footer: FooterBuilder,
    title: &'static str,
    url: &'static str,
}

impl MedalRarityEmbed {
    pub fn new(ranking: &[OsekaiRarityEntry], index: usize, pages: (usize, usize)) -> Self {
        let mut description = String::with_capacity(1024);

        for (i, entry) in ranking.iter().enumerate() {
            let medal_name = entry.medal_name.as_str();
            let tmp = medal_name.cow_replace(' ', "+");
            let url_name = tmp.cow_replace(',', "%2C");

            let _ = writeln!(
                description,
                "**{idx}. [{medal}](https://osekai.net/medals/?medal={url_name})**: `{rarity}%`\n ▸ `{description}`",
                idx = index + i + 1,
                medal = entry.medal_name,
                url_name = url_name,
                rarity = round(entry.possession_percent),
                description = entry.description,
            );
        }

        let title = "Medal Ranking based on rarity";
        let url = "https://osekai.net/rankings/?ranking=Medals&type=Rarity";

        let footer_text = format!(
            "Page {}/{} • Check out osekai.net for more info",
            pages.0, pages.1
        );

        Self {
            description,
            footer: FooterBuilder::new(footer_text),
            title,
            url,
        }
    }
}