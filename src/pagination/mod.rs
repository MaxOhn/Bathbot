mod bg_rankings;
mod command_count;
mod common;
mod country_snipe_list;
mod leaderboard;
mod map;
mod most_played;
mod most_played_common;
mod nochoke;
mod osustats_globals;
mod osustats_list;
mod profile;
mod recent;
mod scores;
mod top;

pub use bg_rankings::BGRankingPagination;
pub use command_count::CommandCountPagination;
pub use common::CommonPagination;
pub use country_snipe_list::*;
pub use leaderboard::LeaderboardPagination;
pub use map::MapPagination;
pub use most_played::MostPlayedPagination;
pub use most_played_common::MostPlayedCommonPagination;
pub use nochoke::NoChokePagination;
pub use osustats_globals::OsuStatsGlobalsPagination;
pub use osustats_list::OsuStatsListPagination;
pub use profile::ProfilePagination;
pub use recent::RecentPagination;
pub use scores::ScoresPagination;
pub use top::TopPagination;

use crate::{embeds::EmbedData, util::numbers, BotResult, Context, CONFIG};

use async_trait::async_trait;
use std::time::Duration;
use tokio::stream::StreamExt;
use twilight_embed_builder::image_source::ImageSource;
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::{
    channel::{Message, Reaction, ReactionType},
    gateway::payload::ReactionAdd,
    id::{EmojiId, UserId},
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
    fn reactions() -> Vec<RequestReactionType> {
        Self::arrow_reactions()
    }
    fn arrow_reactions() -> Vec<RequestReactionType> {
        vec![
            RequestReactionType::Unicode {
                name: "⏮️".to_owned(),
            },
            RequestReactionType::Unicode {
                name: "⏪".to_owned(),
            },
            RequestReactionType::Unicode {
                name: "⏩".to_owned(),
            },
            RequestReactionType::Unicode {
                name: "⏭️".to_owned(),
            },
        ]
    }
    fn arrow_reactions_full() -> Vec<RequestReactionType> {
        vec![
            RequestReactionType::Unicode {
                name: "⏮️".to_owned(),
            },
            RequestReactionType::Unicode {
                name: "⏪".to_owned(),
            },
            RequestReactionType::Unicode {
                name: "◀️".to_owned(),
            },
            RequestReactionType::Unicode {
                name: "▶️".to_owned(),
            },
            RequestReactionType::Unicode {
                name: "⏩".to_owned(),
            },
            RequestReactionType::Unicode {
                name: "⏭️".to_owned(),
            },
        ]
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
    fn thumbnail(&self) -> Option<ImageSource> {
        None
    }
    fn content(&self) -> Option<String> {
        None
    }
    fn main_reactions(&self) -> MainReactions {
        MainReactions::Arrows
    }
    fn process_data(&mut self, _data: &Self::PageData) {}
    async fn change_mode(&mut self) {}
    async fn final_processing(mut self, _ctx: &Context) -> BotResult<()> {
        Ok(())
    }

    // Don't implement anything else
    async fn start(mut self, ctx: &Context, owner: UserId, duration: u64) -> BotResult<()> {
        let mut reaction_stream = {
            let msg = self.msg();
            for emoji in Self::reactions() {
                ctx.http
                    .create_reaction(msg.channel_id, msg.id, emoji)
                    .await?;
            }
            ctx.standby
                .wait_for_reaction_stream(msg.id, move |r: &ReactionAdd| r.0.user_id == owner)
                .timeout(Duration::from_secs(duration))
        };
        while let Some(Ok(reaction)) = reaction_stream.next().await {
            match self.next_page(reaction.0, ctx).await {
                Ok(PageChange::Delete) => return Ok(()),
                Ok(_) => {}
                Err(why) => warn!("Error while paginating: {}", why),
            }
        }
        for emoji in Self::reactions() {
            let msg = self.msg();
            if msg.guild_id.is_none() {
                ctx.http
                    .delete_current_user_reaction(msg.channel_id, msg.id, emoji)
                    .await?;
            } else {
                ctx.http
                    .delete_all_reaction(msg.channel_id, msg.id, emoji)
                    .await?;
            }
        }
        self.final_processing(ctx).await
    }
    async fn next_page(&mut self, reaction: Reaction, ctx: &Context) -> BotResult<PageChange> {
        let change = match self.process_reaction(&reaction.emoji).await {
            PageChange::None => PageChange::None,
            PageChange::Change => {
                let data = self.build_page().await?;
                self.process_data(&data);
                let msg = self.msg();
                let mut update = ctx.http.update_message(msg.channel_id, msg.id);
                if let Some(content) = self.content() {
                    update = update.content(content)?;
                }
                let mut eb = data.build();
                if let Some(thumbnail) = self.thumbnail() {
                    eb = eb.thumbnail(thumbnail);
                }
                update.embed(eb.build()?)?.await?;
                PageChange::Change
            }
            PageChange::Delete => {
                let msg = self.msg();
                ctx.http.delete_message(msg.channel_id, msg.id).await?;
                PageChange::Delete
            }
        };
        Ok(change)
    }

    async fn process_reaction(&mut self, reaction: &ReactionType) -> PageChange {
        let change_result = match self.main_reactions() {
            MainReactions::Arrows => {
                if let ReactionType::Unicode { name } = reaction {
                    self.process_arrows(name.as_str())
                } else {
                    return PageChange::None;
                }
            }
            MainReactions::Modes => {
                if let ReactionType::Custom {
                    name: Some(name), ..
                } = reaction
                {
                    self.process_modes(name.as_str())
                } else {
                    return PageChange::None;
                }
            }
        };
        match change_result {
            Ok(Some(index)) => {
                *self.index_mut() = index;
                self.change_mode().await;
                PageChange::Change
            }
            Ok(None) => PageChange::None,
            Err(page_change) => page_change,
        }
    }

    fn process_modes(&self, reaction: &str) -> Result<Option<usize>, PageChange> {
        let next_index = match reaction {
            "osu_std" => match self.index() {
                0 => None,
                _ => Some(0),
            },
            "osu_taiko" => match self.index() {
                1 => None,
                _ => Some(1),
            },
            "osu_ctb" => match self.index() {
                2 => None,
                _ => Some(2),
            },
            "osu_mania" => match self.index() {
                3 => None,
                _ => Some(3),
            },
            "❌" => return Err(PageChange::Delete),
            _ => None,
        };
        Ok(next_index)
    }

    fn process_arrows(&self, reaction: &str) -> Result<Option<usize>, PageChange> {
        let next_index = match reaction {
            // Move to start
            "⏮️" => match self.index() {
                0 => None,
                _ => Some(0),
            },
            // Move one page left
            "⏪" => match self.index() {
                0 => None,
                idx => Some(idx.saturating_sub(self.multi_step())),
            },
            // Move one index left
            "◀️" => match self.index() {
                0 => None,
                idx => Some(idx.saturating_sub(self.single_step())),
            },
            // Move to specific position
            "*️⃣" => {
                if let Some(index) = self.jump_index() {
                    let i = numbers::last_multiple(self.per_page(), index + 1);
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
                if self.index() == self.last_index() {
                    None
                } else {
                    Some(self.last_index().min(self.index() + self.single_step()))
                }
            }
            // Move one page right
            "⏩" => {
                if self.index() == self.last_index() {
                    None
                } else {
                    Some(self.last_index().min(self.index() + self.multi_step()))
                }
            }
            // Move to end
            "⏭️" => {
                if self.index() == self.last_index() {
                    None
                } else {
                    Some(self.last_index())
                }
            }
            "❌" => return Err(PageChange::Delete),
            _ => None,
        };
        Ok(next_index)
    }
    fn mode_reactions() -> Vec<RequestReactionType> {
        CONFIG
            .get()
            .unwrap()
            .all_modes()
            .iter()
            .map(|(id, name)| (EmojiId(*id), Some(name.to_string())))
            .map(|(id, name)| RequestReactionType::Custom { id, name })
            .collect()
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

pub enum MainReactions {
    Arrows,
    Modes,
}
