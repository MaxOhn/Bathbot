use super::{Pages, Pagination};
use crate::{
    custom_client::{BeatconnectMapSet, BeatconnectSearchParams},
    embeds::MapSearchEmbed,
    BotResult, Context,
};

use async_trait::async_trait;
use std::{collections::BTreeMap, iter::Extend, sync::Arc};
use twilight_model::channel::Message;

pub struct MapSearchPagination {
    msg: Message,
    pages: Pages,
    maps: BTreeMap<usize, BeatconnectMapSet>,
    params: BeatconnectSearchParams,
    reached_last_page: bool,
    ctx: Arc<Context>,
}

impl MapSearchPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        maps: BTreeMap<usize, BeatconnectMapSet>,
        reached_last_page: bool,
        params: BeatconnectSearchParams,
    ) -> Self {
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
            params,
            reached_last_page,
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
                self.params.next_page();

                let search = self
                    .ctx
                    .clients
                    .custom
                    .beatconnect_search(&self.params)
                    .await?;

                if search.is_last_page() {
                    if search.mapsets.is_empty() {
                        let max = *self.maps.keys().last().unwrap();

                        self.pages = Pages::new(10, max + 1);
                        self.pages.index = self.pages.last_index;
                    } else {
                        self.pages.total_pages =
                            self.params.page * 5 + search.mapsets.len() / 10 + 1;

                        self.pages.last_index =
                            (self.params.page * 5 + search.mapsets.len() / 10) * 10;
                    }

                    total_pages = Some(self.pages.total_pages);
                    self.reached_last_page = true;
                } else {
                    self.pages.total_pages += 5;
                    self.pages.last_index += 50;
                }

                let idx = self.pages.index;

                let iter = search
                    .mapsets
                    .into_iter()
                    .enumerate()
                    .map(|(i, s)| (idx + i, s));

                self.maps.extend(iter);
            }

            total_pages
        };

        let embed_fut = MapSearchEmbed::new(
            &self.maps,
            self.params.query.as_str(),
            (self.page(), total_pages),
        );

        Ok(embed_fut.await)
    }
}
