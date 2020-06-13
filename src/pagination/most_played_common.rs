use super::{create_collector, Pages, Pagination};

use crate::{embeds::MostPlayedCommonEmbed, scraper::MostPlayedMap, Error};

use rosu::models::User;
use serenity::{
    async_trait,
    client::Context,
    collector::ReactionCollector,
    model::{channel::Message, id::UserId},
};
use std::collections::HashMap;

pub struct MostPlayedCommonPagination {
    msg: Message,
    collector: ReactionCollector,
    pages: Pages,
    users: HashMap<u32, User>,
    users_count: HashMap<u32, HashMap<u32, u32>>,
    maps: Vec<MostPlayedMap>,
    thumbnail: String,
}

impl MostPlayedCommonPagination {
    pub async fn new(
        ctx: &Context,
        msg: Message,
        author: UserId,
        users: HashMap<u32, User>,
        users_count: HashMap<u32, HashMap<u32, u32>>,
        maps: Vec<MostPlayedMap>,
        thumbnail: String,
    ) -> Self {
        let collector = create_collector(ctx, &msg, author, 60).await;
        Self {
            pages: Pages::new(10, maps.len()),
            msg,
            collector,
            users,
            users_count,
            maps,
            thumbnail,
        }
    }
}

#[async_trait]
impl Pagination for MostPlayedCommonPagination {
    type PageData = MostPlayedCommonEmbed;
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
    fn thumbnail(&self) -> Option<String> {
        Some(self.thumbnail.clone())
    }
    async fn build_page(&mut self) -> Result<Self::PageData, Error> {
        Ok(MostPlayedCommonEmbed::new(
            &self.users,
            &self.maps[self.pages.index..(self.pages.index + 10).min(self.maps.len())],
            &self.users_count,
            self.pages.index,
        )
        .await)
    }
}
