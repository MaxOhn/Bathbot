use super::{Context, Pages, Pagination};

use crate::{embeds::MapEmbed, BotResult};

use rosu_v2::prelude::{Beatmap, Beatmapset, GameMods};
use twilight_model::channel::Message;

pub struct MapPagination {
    msg: Message,
    pages: Pages,
    mapset: Beatmapset,
    maps: Vec<Beatmap>,
    mods: GameMods,
    with_thumbnail: bool,
}

impl MapPagination {
    pub fn new(
        msg: Message,
        mapset: Beatmapset,
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
            mapset,
            maps,
            mods,
            with_thumbnail,
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

    async fn final_processing(mut self, ctx: &Context) -> BotResult<()> {
        // Set maps on garbage collection list if unranked
        for map in self.maps.iter() {
            ctx.map_garbage_collector(map).execute(ctx).await;
        }

        Ok(())
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        MapEmbed::new(
            &self.maps[self.pages.index],
            &self.mapset,
            self.mods,
            self.with_thumbnail,
            (self.pages.index + 1, self.maps.len()),
        )
        .await
    }
}
