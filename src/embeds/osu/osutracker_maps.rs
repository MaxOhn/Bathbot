use std::fmt::Write;

use crate::{
    custom_client::OsuTrackerPpEntry,
    embeds::{Author, Footer},
    util::{constants::OSU_BASE, numbers::with_comma_int},
};

pub struct OsuTrackerMapsEmbed {
    author: Author,
    description: String,
    footer: Footer,
}

impl OsuTrackerMapsEmbed {
    pub fn new(pp: u32, entries: &[OsuTrackerPpEntry], (page, pages): (usize, usize)) -> Self {
        let author_text = format!("Most common maps in top plays: {pp}-{}pp", pp + 100);
        let author = Author::new(author_text).url("https://osutracker.com/stats");

        let footer_text =
            format!("Page {page}/{pages} • Data originates from https://osutracker.com");
        let footer = Footer::new(footer_text);

        let idx = (page - 1) * 10 + 1;
        let mut sizes = Sizes::default();

        for (entry, i) in entries.iter().zip(idx..) {
            sizes.idx = sizes.idx.max(i.to_string().len());

            sizes.count = sizes
                .count
                .max(with_comma_int(entry.count).to_string().len());
        }

        let mut description = String::with_capacity(entries.len() * 100);

        for (entry, i) in entries.iter().zip(idx..) {
            let _ = writeln!(
                description,
                "`{i:>i_len$}.` `{count:>c_len$}` [{name}]({OSU_BASE}b/{map_id})",
                i_len = sizes.idx,
                count = with_comma_int(entry.count).to_string(),
                c_len = sizes.count,
                name = entry.name,
                map_id = entry.map_id,
            );
        }

        description.pop();

        Self {
            author,
            description,
            footer,
        }
    }
}

impl_builder!(OsuTrackerMapsEmbed {
    author,
    footer,
    description
});

#[derive(Default)]
struct Sizes {
    idx: usize,
    count: usize,
}
