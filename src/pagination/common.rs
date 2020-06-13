use super::{Pages, Pagination};

use crate::{embeds::BasicEmbedData, Error};

use rosu::models::{Beatmap, Score, User};
use serenity::{
    async_trait,
    client::Context,
    collector::ReactionCollector,
    model::{channel::Message, id::UserId},
};
use std::collections::HashMap;

pub struct CommonPagination {
    msg: Message,
    collector: ReactionCollector,
    pages: Pages,
    users: HashMap<u32, User>,
    scores: HashMap<u32, Vec<Score>>,
    maps: HashMap<u32, Beatmap>,
    id_pps: Vec<(u32, f32)>,
    thumbnail: String,
}

impl CommonPagination {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        ctx: &Context,
        msg: Message,
        author: UserId,
        users: HashMap<u32, User>,
        scores: HashMap<u32, Vec<Score>>,
        maps: HashMap<u32, Beatmap>,
        id_pps: Vec<(u32, f32)>,
        thumbnail: String,
    ) -> Self {
        let collector = Self::create_collector(ctx, &msg, author, 60).await;
        Self {
            pages: Pages::new(10, scores.len()),
            msg,
            collector,
            users,
            scores,
            maps,
            id_pps,
            thumbnail,
        }
    }
}

#[async_trait]
impl Pagination for CommonPagination {
    type PageData = BasicEmbedData;
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
        Ok(BasicEmbedData::create_common(
            &self.users,
            &self.scores,
            &self.maps,
            &self.id_pps[self.pages.index..(self.pages.index + 10).min(self.id_pps.len())],
            self.pages.index,
        ))
    }
}
