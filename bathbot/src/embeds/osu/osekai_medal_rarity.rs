use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_model::OsekaiRarityEntry;
use bathbot_util::{numbers::round, CowUtils, FooterBuilder};

use crate::pagination::Pages;

#[derive(EmbedData)]
pub struct MedalRarityEmbed {
    description: String,
    footer: FooterBuilder,
    title: &'static str,
    url: &'static str,
}

impl MedalRarityEmbed {
    pub fn new(ranking: &[OsekaiRarityEntry], pages: &Pages) -> Self {
        let mut description = String::with_capacity(1024);

        for (entry, i) in ranking.iter().zip(pages.index + 1..) {
            let medal_name = entry.medal_name.as_str();
            let tmp = medal_name.cow_replace(' ', "+");
            let url_name = tmp.cow_replace(',', "%2C");

            let _ = writeln!(
                description,
                "**{i}. [{medal}](https://osekai.net/medals/?medal={url_name})**: `{rarity}%`\n ▸ `{description}`",
                medal = entry.medal_name,
                rarity = round(entry.possession_percent),
                description = entry.description,
            );
        }

        let title = "Medal Ranking based on rarity";
        let url = "https://osekai.net/rankings/?ranking=Medals&type=Rarity";

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer_text = format!("Page {page}/{pages} • Check out osekai.net for more info");

        Self {
            description,
            footer: FooterBuilder::new(footer_text),
            title,
            url,
        }
    }
}
