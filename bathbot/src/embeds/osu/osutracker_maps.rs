use std::fmt::Write;

use bathbot_macros::EmbedData;

use crate::{
    custom_client::OsuTrackerPpEntry,
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        numbers::WithComma,
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct OsuTrackerMapsEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
}

impl OsuTrackerMapsEmbed {
    pub fn new(pp: u32, entries: &[OsuTrackerPpEntry], pages: &Pages) -> Self {
        let author_text = format!("Most common maps in top plays: {pp}-{}pp", pp + 100);
        let author = AuthorBuilder::new(author_text).url("https://osutracker.com/stats");

        let idx = pages.index + 1;
        let mut sizes = Sizes::default();

        for (entry, i) in entries.iter().zip(idx..) {
            sizes.idx = sizes.idx.max(i.to_string().len());

            sizes.count = sizes
                .count
                .max(WithComma::new(entry.count).to_string().len());
        }

        let mut description = String::with_capacity(entries.len() * 100);

        for (entry, i) in entries.iter().zip(idx..) {
            #[allow(clippy::to_string_in_format_args)]
            let _ = writeln!(
                description,
                "`{i:>i_len$}.` `{count:>c_len$}` [{name}]({OSU_BASE}b/{map_id})",
                i_len = sizes.idx,
                count = WithComma::new(entry.count).to_string(),
                c_len = sizes.count,
                name = entry.name.cow_escape_markdown(),
                map_id = entry.map_id,
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
