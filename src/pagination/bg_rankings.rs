use super::{Pages, Pagination};

use crate::{embeds::BasicEmbedData, Error};

use serenity::{
    async_trait,
    cache::Cache,
    client::Context,
    collector::ReactionCollector,
    http::Http,
    model::{channel::Message, id::UserId},
};
use std::{collections::HashMap, sync::Arc};

pub struct BGRankingPagination {
    msg: Message,
    collector: ReactionCollector,
    pages: Pages,
    author_idx: Option<usize>,
    global: bool,
    scores: Vec<(u64, u32)>,
    usernames: HashMap<u64, String>,
    http: Arc<Http>,
    cache: Arc<Cache>,
}

impl BGRankingPagination {
    pub async fn new(
        ctx: &Context,
        msg: Message,
        author: UserId,
        author_idx: Option<usize>,
        scores: Vec<(u64, u32)>,
        global: bool,
    ) -> Self {
        let collector = Self::create_collector(ctx, &msg, author, 60).await;
        let cache = Arc::clone(&ctx.cache);
        let http = Arc::clone(&ctx.http);
        let per_page = 15;
        Self {
            msg,
            collector,
            pages: Pages::new(per_page, scores.len()),
            author_idx,
            scores,
            usernames: HashMap::with_capacity(per_page),
            global,
            http,
            cache,
        }
    }
}

#[async_trait]
impl Pagination for BGRankingPagination {
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
    fn jump_index(&self) -> Option<usize> {
        self.author_idx
    }
    fn reactions() -> &'static [&'static str] {
        &["⏮️", "⏪", "*️⃣", "⏩", "⏭️"]
    }
    async fn build_page(&mut self) -> Result<Self::PageData, Error> {
        for id in self
            .scores
            .iter()
            .skip(self.index())
            .take(self.per_page())
            .map(|(id, _)| id)
        {
            if !self.usernames.contains_key(id) {
                let name = if let Ok(user) = UserId(*id).to_user((&self.cache, &*self.http)).await {
                    user.name
                } else {
                    String::from("Unknown user")
                };
                self.usernames.insert(*id, name);
            }
        }
        let scores = self
            .scores
            .iter()
            .skip(self.index())
            .take(self.per_page())
            .map(|(id, score)| (self.usernames.get(&id).unwrap(), *score))
            .collect();
        Ok(BasicEmbedData::create_bg_ranking(
            self.author_idx,
            scores,
            self.global,
            self.index() + 1,
            (self.page(), self.total_pages()),
        ))
    }
}
