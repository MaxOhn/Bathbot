use std::fmt::Write;

use bathbot_macros::EmbedData;

use crate::{
    custom_client::OsuTrackerModsEntry,
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        numbers::WithComma,
    },
};

#[derive(EmbedData)]
pub struct OsuTrackerModsEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
}

impl OsuTrackerModsEmbed {
    pub fn new(entries: &[OsuTrackerModsEntry], pages: &Pages) -> Self {
        let author =
            AuthorBuilder::new("Most common mods in top plays").url("https://osutracker.com/stats");

        let idx = pages.index + 1;
        let mut sizes = Sizes::default();

        for (entry, i) in entries.iter().take(10).zip(idx..) {
            sizes.idx_left = sizes.idx_left.max(i.to_string().len());
            sizes.mods_left = sizes.mods_left.max(entry.mods.iter().count());

            sizes.count_left = sizes
                .count_left
                .max(WithComma::new(entry.count).to_string().len());
        }

        for (entry, i) in entries.iter().skip(10).zip(idx + 10..) {
            sizes.idx_right = sizes.idx_right.max(i.to_string().len());
            sizes.mods_right = sizes.mods_right.max(entry.mods.iter().count());

            sizes.count_right = sizes
                .count_right
                .max(WithComma::new(entry.count).to_string().len());
        }

        let mut description = String::with_capacity(entries.len() * 30);

        for (entry, i) in entries.iter().take(10).zip(idx..) {
            #[allow(clippy::to_string_in_format_args)]
            let _ = write!(
                description,
                "`{i:>i_len$}.` `{mods}{pad}` `{count:>c_len$}`",
                i_len = sizes.idx_left,
                mods = entry.mods,
                pad = " ".repeat(2 * (sizes.mods_left - entry.mods.iter().count())),
                count = WithComma::new(entry.count).to_string(),
                c_len = sizes.count_left,
            );

            if let Some(entry) = entries.get(i + 10 - idx) {
                #[allow(clippy::to_string_in_format_args)]
                let _ = write!(
                    description,
                    " | `{i:>i_len$}.` `{mods}{pad}` `{count:>c_len$}`",
                    i = i + 10,
                    i_len = sizes.idx_right,
                    mods = entry.mods,
                    pad = " ".repeat(2 * (sizes.mods_right - entry.mods.iter().count())),
                    count = WithComma::new(entry.count).to_string(),
                    c_len = sizes.count_right,
                );
            }

            description.push('\n');
        }

        description.pop();

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer_text =
            format!("Page {page}/{pages} • Data originates from https://osutracker.com");
        let footer = FooterBuilder::new(footer_text);

        Self {
            author,
            description,
            footer,
        }
    }
}

#[derive(Default)]
struct Sizes {
    idx_left: usize,
    mods_left: usize,
    count_left: usize,
    idx_right: usize,
    mods_right: usize,
    count_right: usize,
}
