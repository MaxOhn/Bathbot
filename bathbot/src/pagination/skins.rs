use bathbot_macros::pagination;
use bathbot_psql::model::configs::SkinEntry;
use twilight_model::channel::message::Embed;

use super::Pages;
use crate::embeds::{EmbedData, SkinsEmbed};

#[pagination(per_page = 20, entries = "entries")]
pub struct SkinsPagination {
    entries: Vec<SkinEntry>,
}

impl SkinsPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        SkinsEmbed::new(&self.entries, pages).build()
    }
}
