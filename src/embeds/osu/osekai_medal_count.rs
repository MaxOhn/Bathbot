use crate::{
    custom_client::OsekaiUserEntry,
    embeds::Footer,
    util::{constants::OSU_BASE, numbers::round, CowUtils},
};

use std::fmt::Write;

pub struct MedalCountEmbed {
    title: &'static str,
    description: String,
    footer: Footer,
}

impl MedalCountEmbed {
    pub fn new(ranking: &[OsekaiUserEntry], index: usize, author_idx: Option<usize>) -> Self {
        let mut description = String::with_capacity(1024);

        for (i, entry) in ranking.iter().enumerate() {
            let idx = index + i;

            let url_name = entry
                .rarest_medal
                .cow_replace(' ', "+")
                .cow_replace(',', "%2C");

            let _ = writeln!(
                description,
                "**{idx}.** :flag_{country}: [{author}**{user}**{author}]({base}u/{user_id}) ▶ \
                `{count}` | `{percent}%` ▶ [{medal}](https://osekai.net/medals/?medal={url_name})",
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

        Self {
            description,
            footer: Footer::new("Check out osekai.net for more info"),
            title: "Medal count leaderboard",
        }
    }
}

impl_builder!(MedalCountEmbed {
    description,
    footer,
    title,
});
