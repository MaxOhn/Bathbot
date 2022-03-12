use std::fmt::Write;

use crate::{
    custom_client::OsuTrackerModsEntry,
    embeds::{Author, Footer},
    util::numbers::with_comma_int,
};

pub struct OsuTrackerModsEmbed {
    author: Author,
    description: String,
    footer: Footer,
}

impl OsuTrackerModsEmbed {
    pub fn new(entries: &[OsuTrackerModsEntry], (page, pages): (usize, usize)) -> Self {
        let author =
            Author::new("Most common mods in top plays").url("https://osutracker.com/stats");

        let footer_text =
            format!("Page {page}/{pages} • Data originates from https://osutracker.com");
        let footer = Footer::new(footer_text);

        let idx = (page - 1) * 20 + 1;
        let mut sizes = Sizes::default();

        for (entry, i) in entries.iter().take(10).zip(idx..) {
            sizes.idx_left = sizes.idx_left.max(i.to_string().len());
            sizes.mods_left = sizes.mods_left.max(entry.mods.iter().count());

            sizes.count_left = sizes
                .count_left
                .max(with_comma_int(entry.count).to_string().len());
        }

        for (entry, i) in entries.iter().skip(10).zip(idx + 10..) {
            sizes.idx_right = sizes.idx_right.max(i.to_string().len());
            sizes.mods_right = sizes.mods_right.max(entry.mods.iter().count());

            sizes.count_right = sizes
                .count_right
                .max(with_comma_int(entry.count).to_string().len());
        }

        let mut description = String::with_capacity(entries.len() * 30);

        for (entry, i) in entries.iter().take(10).zip(idx..) {
            let _ = write!(
                description,
                "`{i:>i_len$}.` `{mods}{pad}` `{count:>c_len$}`",
                i_len = sizes.idx_left,
                mods = entry.mods,
                pad = " ".repeat(2 * (sizes.mods_left - entry.mods.iter().count())),
                count = with_comma_int(entry.count).to_string(),
                c_len = sizes.count_left,
            );

            if let Some(entry) = entries.get(i + 9 - idx) {
                let _ = write!(
                    description,
                    " | `{i:>i_len$}.` `{mods}{pad}` `{count:>c_len$}`",
                    i = i + 10,
                    i_len = sizes.idx_right,
                    mods = entry.mods,
                    pad = " ".repeat(2 * (sizes.mods_right - entry.mods.iter().count())),
                    count = with_comma_int(entry.count).to_string(),
                    c_len = sizes.count_right,
                );
            }

            description.push('\n');
        }

        description.pop();

        Self {
            author,
            description,
            footer,
        }
    }
}

impl_builder!(OsuTrackerModsEmbed {
    author,
    footer,
    description
});

#[derive(Default)]
struct Sizes {
    idx_left: usize,
    mods_left: usize,
    count_left: usize,
    idx_right: usize,
    mods_right: usize,
    count_right: usize,
}
