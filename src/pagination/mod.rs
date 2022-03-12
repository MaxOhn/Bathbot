mod bg_rankings;
mod command_count;
mod common;
mod country_snipe_list;
mod leaderboard;
mod map;
mod map_search;
mod match_compare;
mod medal_recent;
mod medals_common;
mod medals_list;
mod medals_missing;
mod most_played;
mod most_played_common;
mod nochoke;
mod osekai_medal_count;
mod osekai_medal_rarity;
mod osustats_globals;
mod osustats_list;
mod osutracker_mappers;
mod osutracker_maps;
mod osutracker_mapsets;
mod osutracker_mods;
mod pinned;
mod player_snipe_list;
mod profile;
mod ranking;
mod ranking_countries;
mod recent_list;
mod scores;
mod sniped_difference;
mod top;
mod top_if;

use std::{borrow::Cow, time::Duration};

use eyre::Report;
use smallvec::SmallVec;
use tokio::time::sleep;
use tokio_stream::StreamExt;
use twilight_gateway::Event;
use twilight_http::error::ErrorType;
use twilight_model::{
    channel::{Message, Reaction, ReactionType},
    id::{marker::UserMarker, Id},
};

use crate::{
    embeds::EmbedData,
    error::Error,
    util::{numbers, send_reaction, Emote},
    BotResult, Context,
};

pub use self::{
    bg_rankings::BGRankingPagination, command_count::CommandCountPagination,
    common::CommonPagination, country_snipe_list::CountrySnipeListPagination,
    leaderboard::LeaderboardPagination, map::MapPagination, map_search::MapSearchPagination,
    match_compare::MatchComparePagination, medal_recent::MedalRecentPagination,
    medals_common::MedalsCommonPagination, medals_list::MedalsListPagination,
    medals_missing::MedalsMissingPagination, most_played::MostPlayedPagination,
    most_played_common::MostPlayedCommonPagination, nochoke::NoChokePagination,
    osekai_medal_count::MedalCountPagination, osekai_medal_rarity::MedalRarityPagination,
    osustats_globals::OsuStatsGlobalsPagination, osustats_list::OsuStatsListPagination,
    osutracker_mappers::OsuTrackerMappersPagination, osutracker_maps::OsuTrackerMapsPagination,
    osutracker_mapsets::OsuTrackerMapsetsPagination, osutracker_mods::OsuTrackerModsPagination,
    pinned::PinnedPagination, player_snipe_list::PlayerSnipeListPagination,
    profile::ProfilePagination, ranking::RankingPagination,
    ranking_countries::RankingCountriesPagination, recent_list::RecentListPagination,
    scores::ScoresPagination, sniped_difference::SnipedDiffPagination, top::TopPagination,
    top_if::TopIfPagination,
};

type ReactionVec = SmallVec<[Emote; 7]>;
type PaginationResult = Result<(), PaginationError>;

#[derive(Debug, thiserror::Error)]
#[error("pagination error")]
pub enum PaginationError {
    Bot(#[from] Error),
    Http(#[from] twilight_http::Error),
}

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
    fn reactions() -> ReactionVec {
        Self::arrow_reactions()
    }

    fn arrow_reactions() -> ReactionVec {
        smallvec![
            Emote::JumpStart,
            Emote::SingleStepBack,
            Emote::SingleStep,
            Emote::JumpEnd,
        ]
    }

    fn arrow_reactions_full() -> ReactionVec {
        smallvec![
            Emote::JumpStart,
            Emote::MultiStepBack,
            Emote::SingleStepBack,
            Emote::SingleStep,
            Emote::MultiStep,
            Emote::JumpEnd,
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

    fn thumbnail(&self) -> Option<String> {
        None
    }

    fn content(&self) -> Option<Cow<'_, str>> {
        None
    }

    fn process_data(&mut self, _data: &Self::PageData) {}

    async fn final_processing(mut self, _ctx: &Context) -> BotResult<()> {
        Ok(())
    }

    // Don't implement anything else
    async fn start(
        mut self,
        ctx: &Context,
        owner: Id<UserMarker>,
        duration: u64,
    ) -> PaginationResult {
        ctx.store_msg(self.msg().id);

        let reactions = Self::reactions();

        let reaction_stream = {
            let msg = self.msg();
            let msg_id = msg.id;

            for emote in &reactions {
                send_reaction(ctx, msg, *emote).await?;
            }

            ctx.standby
                .wait_for_event_stream(move |event: &Event| match event {
                    Event::ReactionAdd(event) => {
                        event.message_id == msg_id && event.user_id == owner
                    }
                    Event::ReactionRemove(event) => {
                        event.message_id == msg_id && event.user_id == owner
                    }
                    _ => false,
                })
                .map(|event| match event {
                    Event::ReactionAdd(add) => ReactionWrapper::Add(add.0),
                    Event::ReactionRemove(remove) => ReactionWrapper::Remove(remove.0),
                    _ => unreachable!(),
                })
                .timeout(Duration::from_secs(duration))
        };

        tokio::pin!(reaction_stream);

        while let Some(Ok(reaction)) = reaction_stream.next().await {
            if let Err(why) = self.next_page(reaction.into_inner(), ctx).await {
                warn!("{:?}", Report::new(why).wrap_err("error while paginating"));
            }
        }

        let msg = self.msg();

        if !ctx.remove_msg(msg.id) {
            return Ok(());
        }

        let delete_fut = ctx.http.delete_all_reactions(msg.channel_id, msg.id).exec();

        if let Err(why) = delete_fut.await {
            if matches!(why.kind(), ErrorType::Response { status, .. } if status.raw() == 403) {
                sleep(Duration::from_millis(100)).await;

                for emote in &reactions {
                    let request_reaction = emote.request_reaction_type();

                    ctx.http
                        .delete_current_user_reaction(msg.channel_id, msg.id, &request_reaction)
                        .exec()
                        .await?;
                }
            } else {
                return Err(why.into());
            }
        }

        self.final_processing(ctx).await.map_err(From::from)
    }

    async fn next_page(&mut self, reaction: Reaction, ctx: &Context) -> BotResult<()> {
        if self.process_reaction(&reaction.emoji).await == PageChange::Change {
            let data = self.build_page().await?;
            self.process_data(&data);
            let msg = self.msg();
            let mut update = ctx.http.update_message(msg.channel_id, msg.id);
            let content = self.content();

            if let Some(ref content) = content {
                update = update.content(Some(content.as_ref()))?;
            }

            let mut builder = data.into_builder();

            if let Some(thumbnail) = self.thumbnail() {
                builder = builder.thumbnail(thumbnail);
            }

            update.embeds(&[builder.build()])?.exec().await?;
        }

        Ok(())
    }

    async fn process_reaction(&mut self, reaction: &ReactionType) -> PageChange {
        let change_result = match reaction {
            ReactionType::Custom {
                name: Some(name), ..
            } => match name.as_str() {
                // Move to start
                "jump_start" => (self.index() != 0).then(|| 0),
                // Move one page left
                "multi_step_back" => match self.index() {
                    0 => None,
                    idx => Some(idx.saturating_sub(self.multi_step())),
                },
                // Move one index left
                "single_step_back" => match self.index() {
                    0 => None,
                    idx => Some(idx.saturating_sub(self.single_step())),
                },
                // Move to specific position
                "my_position" => {
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
                "single_step" => (self.index() != self.last_index())
                    .then(|| self.last_index().min(self.index() + self.single_step())),
                // Move one page right
                "multi_step" => (self.index() != self.last_index())
                    .then(|| self.last_index().min(self.index() + self.multi_step())),
                // Move to end
                "jump_end" => (self.index() != self.last_index()).then(|| self.last_index()),
                _ => None,
            },
            _ => None,
        };

        match change_result {
            Some(index) => {
                *self.index_mut() = index;

                PageChange::Change
            }
            None => PageChange::None,
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

#[derive(Eq, PartialEq)]
pub enum PageChange {
    None,
    Change,
}

#[derive(Copy, Clone, Debug)]
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

enum ReactionWrapper {
    Add(Reaction),
    Remove(Reaction),
}

impl ReactionWrapper {
    fn into_inner(self) -> Reaction {
        match self {
            Self::Add(r) | Self::Remove(r) => r,
        }
    }
}
