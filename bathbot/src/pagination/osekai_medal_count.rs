use bathbot_macros::pagination;
use bathbot_model::OsekaiUserEntry;
use twilight_model::channel::embed::Embed;

use crate::embeds::{EmbedData, MedalCountEmbed};

use super::Pages;

#[pagination(per_page = 10, entries = "ranking")]
pub struct MedalCountPagination {
    ranking: Vec<OsekaiUserEntry>,
    author_idx: Option<usize>,
}

impl MedalCountPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index();
        let limit = self.ranking.len().min(idx + pages.per_page());

        MedalCountEmbed::new(&self.ranking[idx..limit], self.author_idx, pages).build()
    }
}
