use command_macros::pagination;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::NochokeEntry,
    embeds::{EmbedData, NoChokeEmbed},
    manager::redis::{osu::User, RedisData},
};

use super::Pages;

#[pagination(per_page = 5, entries = "entries")]
pub struct NoChokePagination {
    user: RedisData<User>,
    entries: Vec<NochokeEntry>,
    unchoked_pp: f32,
    rank: Option<u32>,
}

impl NoChokePagination {
    pub async fn build_page(&mut self, pages: &Pages) -> Embed {
        let end_idx = self.entries.len().min(pages.index + pages.per_page);
        let entries = &self.entries[pages.index..end_idx];

        let embed_fut = NoChokeEmbed::new(&self.user, entries, self.unchoked_pp, self.rank, pages);

        embed_fut.await.build()
    }
}
