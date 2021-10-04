use std::fmt::Write;

use crate::{
    custom_client::OsekaiUserEntry,
    embeds::Footer,
    util::{constants::OSU_BASE, numbers::round, CowUtils},
};

pub struct MedalCountEmbed {
    description: String,
    footer: Footer,
    title: &'static str,
    url: &'static str,
}

impl MedalCountEmbed {
    pub fn new(
        ranking: &[OsekaiUserEntry],
        index: usize,
        author_idx: Option<usize>,
        pages: (usize, usize),
    ) -> Self {
        let mut description = String::with_capacity(1024);

        for (i, entry) in ranking.iter().enumerate() {
            let idx = index + i;

            let medal_name = entry.rarest_medal.as_str();
            let tmp = medal_name.cow_replace(' ', "+");
            let url_name = tmp.cow_replace(',', "%2C");

            let _ = writeln!(
                description,
                "**{idx}.** :flag_{country}: [{author}**{user}**{author}]({base}u/{user_id}): \
                `{count}` (`{percent}%`) ▸ [{medal}](https://osekai.net/medals/?medal={url_name})",
                idx = idx + 1,
                country = entry.country_code.to_ascii_lowercase(),
                author = if author_idx == Some(idx) { "__" } else { "" },
                user = entry.username,
                base = OSU_BASE,
                user_id = entry.user_id,
                count = entry.medal_count,
                percent = round(entry.completion),
                medal = entry.rarest_medal,
                url_name = url_name,
            );
        }

        let title = "User Ranking based on amount of owned medals";
        let url = "https://osekai.net/rankings/?ranking=Medals&type=Users";

        let mut footer_text = format!("Page {}/{} • ", pages.0, pages.1);

        if let Some(idx) = author_idx {
            let _ = write!(footer_text, "Your position: {} • ", idx + 1);
        }

        footer_text.push_str("Check out osekai.net for more info");

        Self {
            description,
            footer: Footer::new(footer_text),
            title,
            url,
        }
    }
}

impl_builder!(MedalCountEmbed {
    description,
    footer,
    title,
    url,
});
