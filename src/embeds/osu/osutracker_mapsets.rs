use std::fmt::Write;

use hashbrown::HashMap;

use crate::{
    commands::osu::MapsetEntry,
    custom_client::OsuTrackerMapsetEntry,
    embeds::{Author, Footer},
    util::{constants::OSU_BASE, numbers::with_comma_int},
};

pub struct OsuTrackerMapsetsEmbed {
    author: Author,
    description: String,
    footer: Footer,
}

impl OsuTrackerMapsetsEmbed {
    pub fn new(
        entries: &[OsuTrackerMapsetEntry],
        mapsets: &HashMap<u32, MapsetEntry>,
        (page, pages): (usize, usize),
    ) -> Self {
        let author =
            Author::new("Most common mapsets in top plays").url("https://osutracker.com/stats");

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

        let mut description = String::with_capacity(entries.len() * 140);

        for (entry, i) in entries.iter().zip(idx..) {
            let mapset = mapsets.get(&entry.mapset_id).expect("missing mapset");

            let _ = writeln!(
                description,
                "`{i:>i_len$}.` `{count:>c_len$}` [{name}]({OSU_BASE}s/{mapset_id})\n\
                ⯈ [{creator}]({OSU_BASE}u/{user_id}) • <t:{timestamp}:R>",
                i_len = sizes.idx,
                count = with_comma_int(entry.count).to_string(),
                c_len = sizes.count,
                name = mapset.name,
                mapset_id = entry.mapset_id,
                creator = mapset.creator,
                user_id = mapset.user_id,
                timestamp = mapset.ranked_date.timestamp(),
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

impl_builder!(OsuTrackerMapsetsEmbed {
    author,
    footer,
    description
});

#[derive(Default)]
struct Sizes {
    idx: usize,
    count: usize,
}
