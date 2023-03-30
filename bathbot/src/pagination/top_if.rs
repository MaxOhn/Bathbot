use bathbot_macros::pagination;
use bathbot_model::rosu_v2::user::User;
use rosu_v2::prelude::GameMode;
use twilight_model::channel::message::embed::Embed;

use crate::{
    commands::osu::TopIfEntry,
    embeds::{EmbedData, TopIfEmbed},
    manager::redis::RedisData,
};

use super::Pages;

#[pagination(per_page = 5, entries = "entries")]
pub struct TopIfPagination {
    user: RedisData<User>,
    entries: Vec<TopIfEntry>,
    mode: GameMode,
    pre_pp: f32,
    post_pp: f32,
    rank: Option<u32>,
}

impl TopIfPagination {
    pub async fn build_page(&mut self, pages: &Pages) -> Embed {
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let entries = &self.entries[pages.index()..end_idx];

        let embed_fut = TopIfEmbed::new(
            &self.user,
            entries,
            self.mode,
            self.pre_pp,
            self.post_pp,
            self.rank,
            pages,
        );

        embed_fut.await.build()
    }
}
