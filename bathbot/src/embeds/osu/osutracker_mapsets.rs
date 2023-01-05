use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_model::OsuTrackerMapsetEntry;
use bathbot_util::{
    constants::OSU_BASE, numbers::WithComma, AuthorBuilder, CowUtils, FooterBuilder, IntHasher,
};
use hashbrown::HashMap;

use crate::{commands::osu::MapsetEntry, pagination::Pages};

#[derive(EmbedData)]
pub struct OsuTrackerMapsetsEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
}

impl OsuTrackerMapsetsEmbed {
    pub fn new(
        entries: &[OsuTrackerMapsetEntry],
        mapsets: &HashMap<u32, MapsetEntry, IntHasher>,
        pages: &Pages,
    ) -> Self {
        let author = AuthorBuilder::new("Most common mapsets in top plays")
            .url("https://osutracker.com/stats");

        let idx = pages.index + 1;
        let mut sizes = Sizes::default();

        for (entry, i) in entries.iter().zip(idx..) {
            sizes.idx = sizes.idx.max(i.to_string().len());

            sizes.count = sizes
                .count
                .max(WithComma::new(entry.count).to_string().len());
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
                count = WithComma::new(entry.count).to_string(),
                c_len = sizes.count,
                name = mapset.name.cow_escape_markdown(),
                mapset_id = entry.mapset_id,
                creator = mapset.creator.cow_escape_markdown(),
                user_id = mapset.user_id,
                timestamp = mapset.ranked_date.unix_timestamp(),
            );
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
    idx: usize,
    count: usize,
}
