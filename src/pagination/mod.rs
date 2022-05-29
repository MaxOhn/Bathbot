mod badges;
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
mod osutracker_countrytop;
mod osutracker_mappers;
mod osutracker_maps;
mod osutracker_mapsets;
mod osutracker_mods;
mod player_snipe_list;
mod profile;
mod ranking;
mod ranking_countries;
mod recent_list;
mod scores;
mod sniped_difference;
mod top;
mod top_if;

use std::{borrow::Cow, sync::Arc, time::Duration};

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
    badges::BadgePagination, command_count::CommandCountPagination, common::CommonPagination,
    country_snipe_list::CountrySnipeListPagination, leaderboard::LeaderboardPagination,
    map::MapPagination, map_search::MapSearchPagination, match_compare::MatchComparePagination,
    medal_recent::MedalRecentPagination, medals_common::MedalsCommonPagination,
    medals_list::MedalsListPagination, medals_missing::MedalsMissingPagination,
    most_played::MostPlayedPagination, most_played_common::MostPlayedCommonPagination,
    nochoke::NoChokePagination, osekai_medal_count::MedalCountPagination,
    osekai_medal_rarity::MedalRarityPagination, osustats_globals::OsuStatsGlobalsPagination,
    osustats_list::OsuStatsListPagination, osutracker_countrytop::OsuTrackerCountryTopPagination,
    osutracker_mappers::OsuTrackerMappersPagination, osutracker_maps::OsuTrackerMapsPagination,
    osutracker_mapsets::OsuTrackerMapsetsPagination, osutracker_mods::OsuTrackerModsPagination,
    player_snipe_list::PlayerSnipeListPagination, profile::ProfilePagination,
    ranking::RankingPagination, ranking_countries::RankingCountriesPagination,
    recent_list::RecentListPagination, scores::ScoresPagination,
    sniped_difference::SnipedDiffPagination, top::CondensedTopPagination, top::TopPagination,
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

pub trait BasePagination {
    fn msg(&self) -> &Message;
    fn pages(&self) -> &Pages;
    fn pages_mut(&mut self) -> &mut Pages;
    fn jump_index(&self) -> Option<usize>;

    fn multi_reaction(&self, vec: &mut ReactionVec);
    fn my_pos_reaction(&self, vec: &mut ReactionVec);

    fn single_step(&self) -> usize {
        self.pages().per_page
    }

    fn multi_step(&self) -> usize {
        match self.pages().total_pages {
            0..=8 => self.pages().per_page * 2,
            9..=15 => self.pages().per_page * 3,
            16..=30 => self.pages().per_page * 5,
            _ => self.pages().per_page * 10,
        }
    }

    fn jump_reaction(&self, vec: &mut ReactionVec) {
        vec.push(Emote::JumpStart);
        self.multi_reaction(vec);
        vec.push(Emote::JumpEnd);
    }

    fn single_reaction(&self, vec: &mut ReactionVec) {
        vec.push(Emote::SingleStepBack);
        self.my_pos_reaction(vec);
        vec.push(Emote::SingleStep);
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
}

#[async_trait]
pub trait Pagination: BasePagination + Send + Sync + Sized {
    type PageData: EmbedData + Send;

    // Implement this
    async fn build_page(&mut self) -> BotResult<Self::PageData>;

    // Optionally implement these
    fn content(&self) -> Option<Cow<'_, str>> {
        None
    }

    fn process_data(&mut self, _data: &Self::PageData) {}

    async fn final_processing(mut self, _ctx: &Context) -> BotResult<()> {
        Ok(())
    }

    // Don't implement anything else
    fn start(self, ctx: Arc<Context>, owner: Id<UserMarker>, duration: u64)
    where
        Self: 'static,
    {
        tokio::spawn(async move {
            if let Err(err) = start_pagination(self, &ctx, owner, duration).await {
                warn!("{:?}", Report::new(err));
            }
        });
    }

    fn reactions(&self) -> ReactionVec {
        let mut vec = ReactionVec::new();
        self.jump_reaction(&mut vec);

        vec
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

#[derive(Clone, Debug)]
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

async fn start_pagination<P: Pagination + Send>(
    mut pagination: P,
    ctx: &Context,
    owner: Id<UserMarker>,
    duration: u64,
) -> PaginationResult {
    ctx.store_msg(pagination.msg().id);

    let reactions = pagination.reactions();

    let reaction_stream = {
        let msg = pagination.msg();
        let msg_id = msg.id;

        for emote in &reactions {
            send_reaction(ctx, msg, *emote).await?;
        }

        ctx.standby
            .wait_for_event_stream(move |event: &Event| match event {
                Event::ReactionAdd(event) => event.message_id == msg_id && event.user_id == owner,
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
        if let Err(err) = next_page(&mut pagination, reaction.into_inner(), ctx).await {
            warn!("{:?}", Report::new(err).wrap_err("error while paginating"));
        }
    }

    let msg = pagination.msg();

    if !ctx.remove_msg(msg.id) {
        return Ok(());
    }

    let delete_fut = ctx.http.delete_all_reactions(msg.channel_id, msg.id).exec();

    if let Err(err) = delete_fut.await {
        if matches!(err.kind(), ErrorType::Response { status, .. } if status.get() == 403) {
            sleep(Duration::from_millis(100)).await;

            for emote in &reactions {
                let request_reaction = emote.request_reaction_type();

                ctx.http
                    .delete_current_user_reaction(msg.channel_id, msg.id, &request_reaction)
                    .exec()
                    .await?;
            }
        } else {
            return Err(err.into());
        }
    }

    pagination
        .final_processing(ctx)
        .await
        .map_err(PaginationError::Bot)
}

async fn next_page<P: Pagination>(
    pagination: &mut P,
    reaction: Reaction,
    ctx: &Context,
) -> BotResult<()> {
    if process_reaction(pagination, &reaction.emoji).await == PageChange::Change {
        let data = pagination.build_page().await?;
        pagination.process_data(&data);
        let msg = pagination.msg();
        let mut update = ctx.http.update_message(msg.channel_id, msg.id);
        let content = pagination.content();

        if let Some(ref content) = content {
            update = update.content(Some(content.as_ref()))?;
        }

        let builder = data.build();

        update.embeds(Some(&[builder]))?.exec().await?;
    }

    Ok(())
}

async fn process_reaction<P: Pagination>(
    pagination: &mut P,
    reaction: &ReactionType,
) -> PageChange {
    let change_result = match reaction {
        ReactionType::Custom {
            name: Some(name), ..
        } => match name.as_str() {
            // Move to start
            "jump_start" => (pagination.index() != 0).then(|| 0),
            // Move one page left
            "multi_step_back" => match pagination.index() {
                0 => None,
                idx => Some(idx.saturating_sub(pagination.multi_step())),
            },
            // Move one index left
            "single_step_back" => match pagination.index() {
                0 => None,
                idx => Some(idx.saturating_sub(pagination.single_step())),
            },
            // Move to specific position
            "my_position" => {
                if let Some(index) = pagination.jump_index() {
                    let i = numbers::last_multiple(pagination.per_page(), index + 1);

                    if i != pagination.index() {
                        Some(i)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            // Move one index right
            "single_step" => (pagination.index() != pagination.last_index()).then(|| {
                pagination
                    .last_index()
                    .min(pagination.index() + pagination.single_step())
            }),
            // Move one page right
            "multi_step" => (pagination.index() != pagination.last_index()).then(|| {
                pagination
                    .last_index()
                    .min(pagination.index() + pagination.multi_step())
            }),
            // Move to end
            "jump_end" => {
                (pagination.index() != pagination.last_index()).then(|| pagination.last_index())
            }
            _ => None,
        },
        _ => None,
    };

    match change_result {
        Some(index) => {
            *pagination.index_mut() = index;

            PageChange::Change
        }
        None => PageChange::None,
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
