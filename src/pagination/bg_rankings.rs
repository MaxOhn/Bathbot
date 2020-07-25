use super::{Pages, Pagination};
use crate::{embeds::BGRankingEmbed, BotResult, Context};

use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use twilight::model::{channel::Message, id::UserId};

pub struct BGRankingPagination {
    msg: Message,
    pages: Pages,
    author_idx: Option<usize>,
    global: bool,
    scores: Vec<(u64, u32)>,
    usernames: HashMap<u64, String>,
    ctx: Arc<Context>,
}

impl BGRankingPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        author_idx: Option<usize>,
        scores: Vec<(u64, u32)>,
        global: bool,
    ) -> Self {
        let per_page = 15;
        Self {
            msg,
            pages: Pages::new(per_page, scores.len()),
            author_idx,
            scores,
            usernames: HashMap::with_capacity(per_page),
            global,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for BGRankingPagination {
    type PageData = BGRankingEmbed;
    fn msg(&self) -> &Message {
        &self.msg
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
    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        for id in self
            .scores
            .iter()
            .skip(self.pages.index)
            .take(self.pages.per_page)
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
            .skip(self.pages.index)
            .take(self.pages.per_page)
            .map(|(id, score)| (self.usernames.get(&id).unwrap(), *score))
            .collect();
        Ok(BGRankingEmbed::new(
            self.author_idx,
            scores,
            self.global,
            self.pages.index + 1,
            (self.page(), self.pages.total_pages),
        ))
    }
}
