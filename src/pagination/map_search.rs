use super::{Pages, Pagination};
use crate::{commands::osu::MapSearchArgs, embeds::MapSearchEmbed, BotResult, Context};

use rosu_v2::prelude::{Beatmapset, BeatmapsetSearchResult};
use std::{collections::BTreeMap, iter::Extend, sync::Arc};
use twilight_model::channel::Message;

pub struct MapSearchPagination {
    msg: Message,
    pages: Pages,
    maps: BTreeMap<usize, Beatmapset>,
    search_result: BeatmapsetSearchResult,
    args: MapSearchArgs,
    request_page: usize,
    reached_last_page: bool,
    ctx: Arc<Context>,
}

impl MapSearchPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        maps: BTreeMap<usize, Beatmapset>,
        search_result: BeatmapsetSearchResult,
        args: MapSearchArgs,
    ) -> Self {
        let reached_last_page = maps.len() < 50;

        let pages = if reached_last_page {
            Pages::new(10, maps.len())
        } else {
            Pages {
                index: 0,
                per_page: 10,
                last_index: 50,
                total_pages: 6,
            }
        };

        Self {
            pages,
            msg,
            maps,
            search_result,
            args,
            reached_last_page,
            request_page: 0,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for MapSearchPagination {
    type PageData = MapSearchEmbed;

    fn msg(&self) -> &Message {
        &self.msg
    }

    fn pages(&self) -> Pages {
        self.pages
    }

    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }

    fn single_step(&self) -> usize {
        self.pages.per_page
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let total_pages = if self.reached_last_page {
            Some(self.pages.total_pages)
        } else {
            let count = self
                .maps
                .range(self.pages.index..self.pages.index + self.pages.per_page)
                .count();

            let mut total_pages = None;

            if count < self.pages.per_page {
                let mut next_search_result =
                    self.search_result.get_next(self.ctx.osu()).await.unwrap()?;

                self.request_page += 1;

                if next_search_result.mapsets.len() < 50 {
                    if next_search_result.mapsets.is_empty() {
                        let max = *self.maps.keys().last().unwrap();

                        self.pages = Pages::new(10, max + 1);
                        self.pages.index = self.pages.last_index;
                    } else {
                        let mapsets = next_search_result.mapsets.len();

                        self.pages.total_pages = self.request_page * 5 + mapsets / 10 + 1;
                        self.pages.last_index = (self.request_page * 5 + mapsets / 10) * 10;
                    }

                    total_pages = Some(self.pages.total_pages);
                    self.reached_last_page = true;
                } else {
                    self.pages.total_pages += 5;
                    self.pages.last_index += 50;
                }

                let idx = self.pages.index;

                let iter = next_search_result
                    .mapsets
                    .drain(..)
                    .enumerate()
                    .map(|(i, s)| (idx + i, s));

                self.maps.extend(iter);
                self.search_result = next_search_result;
            }

            total_pages
        };

        Ok(MapSearchEmbed::new(&self.maps, &self.args, (self.page(), total_pages)).await)
    }
}
