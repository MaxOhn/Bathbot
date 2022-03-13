use std::fmt::Write;

use crate::{
    commands::osu::OsuTrackerCountryDetailsCompact,
    custom_client::OsuTrackerCountryScore,
    embeds::{Author, Footer},
    util::{
        constants::OSU_BASE,
        numbers::{round, with_comma_float},
        osu::flag_url,
        CowUtils,
    },
};

pub struct OsuTrackerCountryTopEmbed {
    author: Author,
    description: String,
    footer: Footer,
    title: String,
}

impl OsuTrackerCountryTopEmbed {
    pub fn new(
        details: &OsuTrackerCountryDetailsCompact,
        scores: &[OsuTrackerCountryScore],
        (page, pages): (usize, usize),
    ) -> Self {
        let author_text = format!(
            "{country}'{genitive} top scores",
            country = details.country,
            genitive = if details.country.ends_with('s') {
                ""
            } else {
                "s"
            },
        );

        let author_url = format!("https://osutracker.com/country/{}", details.code);

        let author = Author::new(author_text)
            .url(author_url)
            .icon_url(flag_url(details.code.as_str()));

        let footer_text =
            format!("Page {page}/{pages} • Data originates from https://osutracker.com");
        let footer = Footer::new(footer_text);

        let title = format!("Total PP: {}pp", with_comma_float(details.pp));

        let idx = (page - 1) * 10 + 1;

        let mut description = String::with_capacity(scores.len() * 160);

        for (score, i) in scores.iter().zip(idx..) {
            let _ = writeln!(
                description,
                "**{i}.** [{map_name}]({OSU_BASE}b/{map_id}) **+{mods}**\n\
                | by __{user}__ • **{pp}pp** • {acc}% • <t:{timestamp}:R>",
                map_name = score.name,
                map_id = score.map_id,
                mods = score.mods,
                user = score.player.cow_replace('_', "\\_"),
                pp = round(score.pp),
                acc = round(score.acc),
                timestamp = score.created_at.timestamp(),
            );
        }

        Self {
            author,
            description,
            footer,
            title,
        }
    }
}

impl_builder!(OsuTrackerCountryTopEmbed {
    author,
    footer,
    description,
    title,
});
