use std::fmt::Write;

use command_macros::EmbedData;
use hashbrown::HashMap;

use crate::{
    commands::osu::MapsetEntry,
    custom_client::OsuTrackerMapsetEntry,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        numbers::with_comma_int,
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct OsuTrackerMapsetsEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
}

impl OsuTrackerMapsetsEmbed {
    pub fn new(
        entries: &[OsuTrackerMapsetEntry],
        mapsets: &HashMap<u32, MapsetEntry>,
        (page, pages): (usize, usize),
    ) -> Self {
        let author = AuthorBuilder::new("Most common mapsets in top plays")
            .url("https://osutracker.com/stats");

        let footer_text =
            format!("Page {page}/{pages} • Data originates from https://osutracker.com");
        let footer = FooterBuilder::new(footer_text);

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

            #[allow(clippy::to_string_in_format_args)]
            let _ = writeln!(
                description,
                "`{i:>i_len$}.` `{count:>c_len$}` [{name}]({OSU_BASE}s/{mapset_id})\n\
                ⯈ [{creator}]({OSU_BASE}u/{user_id}) • <t:{timestamp}:R>",
                i_len = sizes.idx,
                count = with_comma_int(entry.count).to_string(),
                c_len = sizes.count,
                name = mapset.name.cow_escape_markdown(),
                mapset_id = entry.mapset_id,
                creator = mapset.creator.cow_escape_markdown(),
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

#[derive(Default)]
struct Sizes {
    idx: usize,
    count: usize,
}
