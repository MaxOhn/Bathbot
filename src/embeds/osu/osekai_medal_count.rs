use std::fmt::Write;

use command_macros::EmbedData;

use crate::{
    custom_client::OsekaiUserEntry,
    pagination::Pages,
    util::{builder::FooterBuilder, constants::OSU_BASE, numbers::round, CowUtils},
};

#[derive(EmbedData)]
pub struct MedalCountEmbed {
    description: String,
    footer: FooterBuilder,
    title: &'static str,
    url: &'static str,
}

impl MedalCountEmbed {
    pub fn new(ranking: &[OsekaiUserEntry], author_idx: Option<usize>, pages: &Pages) -> Self {
        let mut description = String::with_capacity(1024);

        for (entry, idx) in ranking.iter().zip(pages.index..) {
            let medal_name = entry.rarest_medal.as_str();
            let tmp = medal_name.cow_replace(' ', "+");
            let url_name = tmp.cow_replace(',', "%2C");

            let _ = writeln!(
                description,
                "**{i}.** :flag_{country}: [{author}**{user}**{author}]({OSU_BASE}u/{user_id}): \
                `{count}` (`{percent}%`) ▸ [{medal}](https://osekai.net/medals/?medal={url_name})",
                i = idx + 1,
                country = entry.country_code.to_ascii_lowercase(),
                author = if author_idx == Some(idx) { "__" } else { "" },
                user = entry.username.cow_escape_markdown(),
                user_id = entry.user_id,
                count = entry.medal_count,
                percent = round(entry.completion),
                medal = entry.rarest_medal,
            );
        }

        let title = "User Ranking based on amount of owned medals";
        let url = "https://osekai.net/rankings/?ranking=Medals&type=Users";

        let page = pages.curr_page();
        let pages = pages.last_page();
        let mut footer_text = format!("Page {page}/{pages} • ");

        if let Some(idx) = author_idx {
            let _ = write!(footer_text, "Your position: {} • ", idx + 1);
        }

        footer_text.push_str("Check out osekai.net for more info");

        Self {
            description,
            footer: FooterBuilder::new(footer_text),
            title,
            url,
        }
    }
}
