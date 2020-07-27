use super::{Pages, Pagination};

use crate::{embeds::MapEmbed, BotResult, Context};

use async_trait::async_trait;
use rosu::models::{Beatmap, GameMods};
use std::sync::Arc;
use twilight::model::channel::Message;

pub struct MapPagination {
    msg: Message,
    pages: Pages,
    maps: Vec<Beatmap>,
    mods: GameMods,
    with_thumbnail: bool,
    ctx: Arc<Context>,
}

impl MapPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        maps: Vec<Beatmap>,
        mods: GameMods,
        start_idx: usize,
        with_thumbnail: bool,
    ) -> Self {
        let mut pages = Pages::new(1, maps.len());
        pages.index = start_idx;
        Self {
            msg,
            pages,
            maps,
            mods,
            with_thumbnail,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for MapPagination {
    type PageData = MapEmbed;
    fn msg(&self) -> &Message {
        &self.msg
    }
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    fn multi_step(&self) -> usize {
        3
    }
    fn reactions() -> &'static [&'static str] {
        &["⏮️", "⏪", "◀️", "▶️", "⏩", "⏭️"]
    }
    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        MapEmbed::new(
            &self.ctx,
            &self.maps[self.pages.index],
            self.mods,
            self.with_thumbnail,
            (self.pages.index + 1, self.maps.len()),
        )
        .await
    }
}
