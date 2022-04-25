use std::sync::Arc;

use command_macros::BasePagination;
use hashbrown::HashMap;
use twilight_model::{channel::Message, id::Id};

use crate::{embeds::BGRankingEmbed, util::Emote, BotResult, Context};

use super::{Pages, Pagination, ReactionVec};

#[derive(BasePagination)]
#[pagination(single_step = 15)]
pub struct BGRankingPagination {
    msg: Message,
    pages: Pages,
    author_idx: Option<usize>,
    scores: Vec<(u64, u32)>,
    usernames: HashMap<u64, String>,
    global: bool,
    ctx: Arc<Context>,
}

impl BGRankingPagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        author_idx: Option<usize>,
        scores: Vec<(u64, u32)>,
        usernames: HashMap<u64, String>,
        global: bool,
    ) -> Self {
        Self {
            msg,
            pages: Pages::new(15, scores.len()),
            author_idx,
            scores,
            usernames,
            global,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for BGRankingPagination {
    type PageData = BGRankingEmbed;

    fn jump_index(&self) -> Option<usize> {
        self.author_idx
    }

    fn reactions() -> ReactionVec {
        smallvec::smallvec![
            Emote::JumpStart,
            Emote::SingleStepBack,
            Emote::MyPosition,
            Emote::SingleStep,
            Emote::JumpEnd,
        ]
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        for &id in self
            .scores
            .iter()
            .skip(self.pages.index)
            .take(self.pages.per_page)
            .map(|(id, _)| id)
        {
            if !self.usernames.contains_key(&id) {
                let name = self
                    .ctx
                    .cache
                    .user(Id::new(id), |user| user.name.clone())
                    .unwrap_or_else(|_| "Unknown user".to_owned());

                self.usernames.insert(id, name);
            }
        }

        let scores = self
            .scores
            .iter()
            .skip(self.pages.index)
            .take(self.pages.per_page)
            .map(|(id, score)| (self.usernames.get(id).unwrap(), *score))
            .collect();

        Ok(BGRankingEmbed::new(
            self.author_idx,
            scores,
            self.pages.index + 1,
            self.global,
            (self.page(), self.pages.total_pages),
        ))
    }
}
