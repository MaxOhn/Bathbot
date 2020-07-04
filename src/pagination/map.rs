use super::{create_collector, Pages, Pagination};

use crate::embeds::MapEmbed;

use failure::Error;
use rosu::models::{Beatmap, GameMods};
use serenity::{
    async_trait,
    client::Context,
    collector::ReactionCollector,
    model::{channel::Message, id::UserId},
    prelude::{RwLock, TypeMap},
};
use std::sync::Arc;

pub struct MapPagination {
    msg: Message,
    collector: ReactionCollector,
    pages: Pages,
    maps: Vec<Beatmap>,
    mods: GameMods,
    with_thumbnail: bool,
    data: Arc<RwLock<TypeMap>>,
}

impl MapPagination {
    pub async fn new(
        ctx: &Context,
        msg: Message,
        author: UserId,
        maps: Vec<Beatmap>,
        mods: GameMods,
        start_idx: usize,
        with_thumbnail: bool,
    ) -> Self {
        let collector = create_collector(ctx, &msg, author, 90).await;
        let data = Arc::clone(&ctx.data);
        let mut pages = Pages::new(1, maps.len());
        pages.index = start_idx;
        Self {
            msg,
            collector,
            pages,
            maps,
            mods,
            with_thumbnail,
            data,
        }
    }
}

#[async_trait]
impl Pagination for MapPagination {
    type PageData = MapEmbed;
    fn msg(&mut self) -> &mut Message {
        &mut self.msg
    }
    fn collector(&mut self) -> &mut ReactionCollector {
        &mut self.collector
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
    async fn build_page(&mut self) -> Result<Self::PageData, Error> {
        MapEmbed::new(
            &self.maps[self.pages.index],
            self.mods,
            self.with_thumbnail,
            (self.pages.index + 1, self.maps.len()),
            Arc::clone(&self.data),
        )
        .await
    }
}
