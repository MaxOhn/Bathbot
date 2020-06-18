mod bg_rankings;
mod command_count;
mod common;
mod leaderboard;
mod most_played;
mod most_played_common;
mod nochoke;
mod recent;
mod top;

pub use bg_rankings::BGRankingPagination;
pub use command_count::CommandCountPagination;
pub use common::CommonPagination;
pub use leaderboard::LeaderboardPagination;
pub use most_played::MostPlayedPagination;
pub use most_played_common::MostPlayedCommonPagination;
pub use nochoke::NoChokePagination;
pub use recent::RecentPagination;
pub use top::TopPagination;

use crate::{embeds::EmbedData, util::numbers};

use failure::Error;
use serenity::{
    async_trait,
    cache::Cache,
    client::Context,
    collector::{ReactionAction, ReactionCollector},
    http::Http,
    model::{
        channel::{Message, ReactionType},
        id::UserId,
    },
};
use std::{convert::TryFrom, sync::Arc, time::Duration};
use tokio::stream::StreamExt;

#[async_trait]
pub trait Pagination: Sync + Sized {
    type PageData: EmbedData;

    // Make these point to the corresponding struct fields
    fn msg(&mut self) -> &mut Message;
    fn collector(&mut self) -> &mut ReactionCollector;
    fn pages(&self) -> Pages;
    fn pages_mut(&mut self) -> &mut Pages;

    // Implement this
    async fn build_page(&mut self) -> Result<Self::PageData, Error>;

    // Optionally implement these
    fn reactions() -> &'static [&'static str] {
        &["⏮️", "⏪", "⏩", "⏭️"]
    }
    fn jump_index(&self) -> Option<usize> {
        None
    }
    fn thumbnail(&self) -> Option<String> {
        None
    }
    fn content(&self) -> Option<String> {
        None
    }
    fn process_data(&mut self, _data: &Self::PageData) {}
    async fn final_processing(mut self, _cache: Arc<Cache>, _http: Arc<Http>) -> Result<(), Error> {
        Ok(())
    }

    // Don't implement anything else
    async fn start(mut self, cache: Arc<Cache>, http: Arc<Http>) -> Result<(), Error> {
        let reactions = Self::reactions();
        for &reaction in reactions.iter() {
            let reaction_type = ReactionType::try_from(reaction).unwrap();
            self.msg().react((&cache, &*http), reaction_type).await?;
        }
        while let Some(reaction) = self.collector().next().await {
            match self.next_page(reaction, &cache, &http).await {
                Ok(Some(data)) => {
                    let content = self.content();
                    let thumbnail = self.thumbnail();
                    self.msg()
                        .edit((&cache, &*http), |m| {
                            if let Some(content) = content {
                                m.content(content);
                            }
                            m.embed(|e| {
                                if let Some(ref thumbnail) = thumbnail {
                                    e.thumbnail(thumbnail);
                                }
                                data.build(e)
                            })
                        })
                        .await?;
                }
                Ok(None) => {}
                Err(why) => warn!("Error while paginating: {}", why),
            }
        }
        for &reaction in reactions.iter() {
            let r = ReactionType::try_from(reaction).unwrap();
            let msg = self.msg();
            if msg.is_private() {
                msg.channel_id
                    .delete_reaction(&http, msg.id, None, r)
                    .await?;
            } else {
                msg.delete_reaction_emoji((&cache, &*http), r).await?;
            }
        }
        self.final_processing(cache, http).await
    }
    async fn next_page(
        &mut self,
        reaction: Arc<ReactionAction>,
        cache: &Arc<Cache>,
        http: &Http,
    ) -> Result<Option<Self::PageData>, Error> {
        if let ReactionAction::Added(reaction) = &*reaction {
            if let ReactionType::Unicode(ref reaction) = reaction.emoji {
                return match self.process_reaction(reaction.as_str()) {
                    PageChange::None => Ok(None),
                    PageChange::Change => {
                        let data = self.build_page().await.map(Some);
                        if let Ok(Some(ref data)) = data {
                            self.process_data(data);
                        }
                        data
                    }
                    PageChange::Delete => {
                        self.msg().delete((cache, http)).await?;
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

pub async fn create_collector(
    ctx: &Context,
    msg: &Message,
    author: UserId,
    sec_duration: u64,
) -> ReactionCollector {
    msg.await_reactions(ctx)
        .timeout(Duration::from_secs(sec_duration))
        .author_id(author)
        .await
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
