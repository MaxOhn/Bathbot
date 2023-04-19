use std::{collections::BTreeMap, iter::Extend};

use eyre::Result;
use rosu_v2::prelude::{Beatmapset, BeatmapsetSearchResult};
use twilight_model::channel::message::embed::Embed;

use super::{Pages, PaginationBuilder, PaginationKind};
use crate::{
    commands::osu::Search,
    embeds::{EmbedData, MapSearchEmbed},
    Context,
};

pub struct MapSearchPagination {
    maps: BTreeMap<usize, Beatmapset>,
    search_result: BeatmapsetSearchResult,
    args: Search,
}

impl MapSearchPagination {
    pub fn builder(
        maps: BTreeMap<usize, Beatmapset>,
        search_result: BeatmapsetSearchResult,
        args: Search,
    ) -> PaginationBuilder {
        let total = search_result.total as usize;

        let pagination = Self {
            maps,
            search_result,
            args,
        };

        let kind = PaginationKind::MapSearch(Box::new(pagination));
        let pages = Pages::new(10, total);

        PaginationBuilder::new(kind, pages)
    }

    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let count = self
            .maps
            .range(pages.index()..pages.index() + pages.per_page())
            .count();

        if count < pages.per_page() {
            let next_fut = self.search_result.get_next(ctx.osu());

            if let Some(mut next_search_result) = next_fut.await.transpose()? {
                let idx = pages.index();

                let iter = next_search_result
                    .mapsets
                    .drain(..)
                    .enumerate()
                    .map(|(i, s)| (idx + i, s));

                self.maps.extend(iter);
                self.search_result = next_search_result;
            }
        }

        Ok(MapSearchEmbed::new(&self.maps, &self.args, pages).build())
    }
}
