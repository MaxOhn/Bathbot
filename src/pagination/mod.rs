mod bg_rankings;
mod command_count;
mod leaderboard;
mod most_played;
mod nochoke;
mod recent;
mod top;

pub use bg_rankings::BGRankingPagination;
pub use command_count::CommandCountPagination;
pub use leaderboard::LeaderboardPagination;
pub use most_played::MostPlayedPagination;
pub use nochoke::NoChokePagination;
pub use recent::RecentPagination;
pub use top::TopPagination;

use crate::{embeds::BasicEmbedData, util::numbers, Error};

use serenity::{
    async_trait,
    cache::Cache,
    collector::ReactionAction,
    http::Http,
    model::channel::{Message, ReactionType},
};
use std::sync::Arc;

use crate::embeds::RecentData;

pub trait Test {
    fn test(&self) {}
}
impl Test for Result<BasicEmbedData, Error> {}
impl Test for Result<RecentData, Error> {}

#[async_trait]
pub trait Pagination: Sync {
    // Implement these three
    fn pages(&self) -> Pages;
    fn pages_mut(&mut self) -> &mut Pages;
    async fn build_page(&mut self, x: &mut impl Test);

    // Optionally implement this
    fn jump_index(&self) -> Option<usize> {
        None
    }

    // Don't implement anything else
    async fn _build_page(&mut self) -> Result<BasicEmbedData, Error> {
        let mut res = Ok(BasicEmbedData::default());
        self.build_page(&mut res).await;
        res
    }

    async fn next_page(
        &mut self,
        reaction: Arc<ReactionAction>,
        msg: &Message,
        cache: &Arc<Cache>,
        http: &Http,
    ) -> Result<Option<BasicEmbedData>, Error> {
        if let ReactionAction::Added(reaction) = &*reaction {
            if let ReactionType::Unicode(ref reaction) = reaction.emoji {
                return match self.process_reaction(reaction.as_str()) {
                    PageChange::None => Ok(None),
                    PageChange::Change => self._build_page().await.map(Some),
                    PageChange::Delete => {
                        msg.delete((cache, http)).await?;
                        Ok(None)
                    }
                };
            }
        }
        Ok(None)
    }

    fn process_reaction(&mut self, reaction: &str) -> PageChange {
        let next_index = match reaction {
            // Move to start
            "⏮️" => {
                if self.index() > 0 {
                    Some(0)
                } else {
                    None
                }
            }
            // Move one page left
            "⏪" => self.index().checked_sub(self.per_page()),
            // Move one index left
            "◀️" => self.index().checked_sub(1),
            // Move to specific position
            "*️⃣" => {
                if let Some(index) = self.jump_index() {
                    let i = numbers::last_multiple(self.per_page(), index);
                    if i != self.index() {
                        Some(i)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            // Move one index right
            "▶️" => {
                if self.index() < self.last_index() {
                    Some(self.index() + 1)
                } else {
                    None
                }
            }
            // Move one page right
            "⏩" => {
                let index = self.index() + self.per_page();
                if index <= self.last_index() {
                    Some(index)
                } else {
                    None
                }
            }
            // Move to end
            "⏭️" => {
                if self.index() < self.last_index() {
                    Some(self.last_index())
                } else {
                    None
                }
            }
            "❌" => return PageChange::Delete,
            _ => None,
        };
        if let Some(index) = next_index {
            *self.index_mut() = index;
            PageChange::Change
        } else {
            PageChange::None
        }
    }

    fn index(&self) -> usize {
        self.pages().index
    }
    fn last_index(&self) -> usize {
        self.pages().last_index
    }
    fn per_page(&self) -> usize {
        self.pages().per_page
    }
    fn total_pages(&self) -> usize {
        self.pages().total_pages
    }
    fn index_mut(&mut self) -> &mut usize {
        &mut self.pages_mut().index
    }
    fn page(&self) -> usize {
        self.index() / self.per_page() + 1
    }
}

pub enum PageChange {
    None,
    Change,
    Delete,
}

#[derive(Copy, Clone)]
pub struct Pages {
    index: usize,
    last_index: usize,
    per_page: usize,
    total_pages: usize,
}

impl Pages {
    /// `per_page`: How many entries per page
    ///
    /// `amount`: How many entries in total
    pub fn new(per_page: usize, amount: usize) -> Self {
        Self {
            index: 0,
            per_page,
            total_pages: numbers::div_euclid(per_page, amount),
            last_index: numbers::last_multiple(per_page, amount),
        }
    }
}
