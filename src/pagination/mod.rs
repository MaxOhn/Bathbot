mod bg_rankings;
mod command_count;
mod common;
mod leaderboard;
mod map;
mod most_played;
mod most_played_common;
mod nochoke;
mod osustats_globals;
mod recent;
mod top;

pub use bg_rankings::BGRankingPagination;
pub use command_count::CommandCountPagination;
pub use common::CommonPagination;
pub use leaderboard::LeaderboardPagination;
pub use map::MapPagination;
pub use most_played::MostPlayedPagination;
pub use most_played_common::MostPlayedCommonPagination;
pub use nochoke::NoChokePagination;
pub use osustats_globals::OsuStatsGlobalsPagination;
pub use recent::RecentPagination;
pub use top::TopPagination;

use crate::{embeds::EmbedData, util::numbers, BotResult, Context};

use async_trait::async_trait;
use std::time::Duration;
use tokio::stream::StreamExt;
use twilight::model::{
    channel::{Message, Reaction, ReactionType},
    gateway::payload::ReactionAdd,
    id::UserId,
};

#[async_trait]
pub trait Pagination: Sync + Sized {
    type PageData: EmbedData;

    // Make these point to the corresponding struct fields
    fn msg(&self) -> &Message;
    fn pages(&self) -> Pages;
    fn pages_mut(&mut self) -> &mut Pages;

    // Implement this
    async fn build_page(&mut self) -> BotResult<Self::PageData>;

    // Optionally implement these
    fn reactions() -> &'static [&'static str] {
        &["⏮️", "⏪", "⏩", "⏭️"]
    }
    fn single_step(&self) -> usize {
        1
    }
    fn multi_step(&self) -> usize {
        self.pages().per_page
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
    async fn final_processing(mut self, _ctx: &Context) -> BotResult<()> {
        Ok(())
    }

    // Don't implement anything else
    async fn start(mut self, ctx: &Context, owner: UserId, duration: u64) -> BotResult<()> {
        let reactions = Self::reactions();
        let mut reaction_stream = {
            let msg = self.msg();
            for &reaction in reactions.iter() {
                let emote = ReactionType::Unicode {
                    name: reaction.to_string(),
                };
                ctx.http
                    .create_reaction(msg.channel_id, msg.id, emote)
                    .await?;
            }
            ctx.standby
                .wait_for_reaction_stream(msg.id, move |r: &ReactionAdd| r.0.user_id == owner)
                .timeout(Duration::from_secs(duration))
        };
        while let Some(Ok(reaction)) = reaction_stream.next().await {
            match self.next_page(reaction.0, ctx).await {
                Ok(Some(data)) => {
                    let msg = self.msg();
                    let mut update = ctx.http.update_message(msg.channel_id, msg.id);
                    if let Some(content) = self.content() {
                        update = update.content(content)?;
                    }
                    let mut eb = data.build();
                    if let Some(thumbnail) = self.thumbnail() {
                        eb = eb.thumbnail(thumbnail);
                    }
                    update.embed(eb.build())?.await?;
                }
                Ok(None) => {}
                Err(why) => warn!("Error while paginating: {}", why),
            }
        }
        for &reaction in reactions.iter() {
            let r = ReactionType::Unicode {
                name: reaction.to_string(),
            };
            let msg = self.msg();
            if msg.guild_id.is_none() {
                ctx.http
                    .delete_current_user_reaction(msg.channel_id, msg.id, r)
                    .await?;
            } else {
                ctx.http
                    .delete_all_reaction(msg.channel_id, msg.id, r)
                    .await?;
            }
        }
        self.final_processing(ctx).await
    }
    async fn next_page(
        &mut self,
        reaction: Reaction,
        ctx: &Context,
    ) -> BotResult<Option<Self::PageData>> {
        if let ReactionType::Unicode { name: reaction } = reaction.emoji {
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
                    let msg = self.msg();
                    ctx.http.delete_message(msg.channel_id, msg.id).await?;
                    Ok(None)
                }
            };
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
            "⏪" => self.index().checked_sub(self.multi_step()),
            // Move one index left
            "◀️" => self.index().checked_sub(self.single_step()),
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
                let next = self.index() + self.single_step();
                if next <= self.last_index() {
                    Some(next)
                } else {
                    None
                }
            }
            // Move one page right
            "⏩" => {
                let next = self.index() + self.multi_step();
                if next <= self.last_index() {
                    Some(next)
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
