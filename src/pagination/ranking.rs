use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::Arc,
};

use command_macros::BasePagination;
use eyre::Report;
use rosu_v2::prelude::Rankings;
use twilight_model::{channel::Message, id::Id};

use crate::{
    commands::osu::UserValue,
    embeds::{RankingEmbed, RankingEntry, RankingKindData},
    util::Emote,
    BotResult, Context,
};

use super::{Pages, Pagination, ReactionVec};

type Users = BTreeMap<usize, RankingEntry>;

#[derive(BasePagination)]
#[pagination(single_step = 20, multi_step = 200)]
pub struct RankingPagination {
    msg: Message,
    pages: Pages,
    ctx: Arc<Context>,
    users: Users,
    total: usize,
    author_idx: Option<usize>,
    ranking_kind_data: RankingKindData,
}

impl RankingPagination {
    pub fn new(
        msg: Message,
        ctx: Arc<Context>,
        total: usize,
        users: Users,
        author_idx: Option<usize>,
        ranking_kind_data: RankingKindData,
    ) -> Self {
        Self {
            pages: Pages::new(20, total),
            msg,
            ctx,
            users,
            total,
            author_idx,
            ranking_kind_data,
        }
    }

    fn extend_from_ranking(&mut self, ranking: Rankings, offset: usize) {
        let iter = ranking.ranking.into_iter().enumerate().map(|(i, user)| {
            let stats = user.statistics.as_ref().unwrap();

            let value = match self.ranking_kind_data {
                RankingKindData::PpCountry { .. } | RankingKindData::PpGlobal { .. } => {
                    UserValue::PpU32(stats.pp.round() as u32)
                }
                RankingKindData::RankedScore { .. } => UserValue::Amount(stats.ranked_score),
                _ => unreachable!(),
            };

            let entry = RankingEntry {
                value,
                name: user.username,
                country: Some(user.country_code.into()),
            };

            (offset * 50 + i, entry)
        });

        self.users.extend(iter);
    }

    async fn assure_present_users(&mut self, page: usize) -> BotResult<()> {
        let count = self
            .users
            .range(self.pages.index..self.pages.index + self.pages.per_page)
            .count();

        if count < self.pages.per_page && count < self.total - self.pages.index {
            let offset = page - 1;
            let page = page as u32;
            let kind = &self.ranking_kind_data;

            match kind {
                RankingKindData::BgScores { scores, .. } => {
                    for i in
                        self.pages.index..(self.pages.index + self.pages.per_page).min(self.total)
                    {
                        if let Entry::Vacant(entry) = self.users.entry(i) {
                            let (id, score) = scores[i];
                            let id = Id::new(id);

                            let name = match self.ctx.psql().get_user_osu(id).await {
                                Ok(Some(osu)) => osu.into_username(),
                                Ok(None) => self
                                    .ctx
                                    .cache
                                    .user(id, |user| user.name.clone())
                                    .unwrap_or_else(|_| "Unknown user".to_owned())
                                    .into(),
                                Err(err) => {
                                    let report =
                                        Report::new(err).wrap_err("failed to get osu user");
                                    warn!("{report:?}");

                                    self.ctx
                                        .cache
                                        .user(id, |user| user.name.clone())
                                        .unwrap_or_else(|_| "Unknown user".to_owned())
                                        .into()
                                }
                            };

                            entry.insert(RankingEntry {
                                value: UserValue::Amount(score as u64),
                                name,
                                country: None,
                            });
                        }
                    }
                }
                RankingKindData::PpCountry {
                    mode,
                    country_code: country,
                    ..
                } => {
                    let ranking = self
                        .ctx
                        .osu()
                        .performance_rankings(*mode)
                        .country(country.as_str())
                        .page(page)
                        .await?;

                    self.extend_from_ranking(ranking, offset);
                }
                RankingKindData::PpGlobal { mode } => {
                    let ranking = self
                        .ctx
                        .osu()
                        .performance_rankings(*mode)
                        .page(page)
                        .await?;

                    self.extend_from_ranking(ranking, offset);
                }
                RankingKindData::RankedScore { mode } => {
                    let ranking = self.ctx.osu().score_rankings(*mode).page(page).await?;
                    self.extend_from_ranking(ranking, offset);
                }
                _ => {} // other data does not come paginated
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Pagination for RankingPagination {
    type PageData = RankingEmbed;

    fn reactions() -> ReactionVec {
        smallvec::smallvec![
            Emote::JumpStart,
            Emote::MultiStepBack,
            Emote::SingleStepBack,
            Emote::MyPosition,
            Emote::SingleStep,
            Emote::MultiStep,
            Emote::JumpEnd,
        ]
    }

    fn jump_index(&self) -> Option<usize> {
        self.author_idx
    }

    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        let idx = self.pages.index.saturating_sub(1);
        let mut page = ((idx - idx % 50) + 50) / 50;
        page += self.users.contains_key(&idx) as usize;

        self.assure_present_users(page).await?;

        // Handle edge cases like idx=140;total=151 where two pages have to be requested at once
        self.assure_present_users(page + 1).await?;

        let pages = (self.page(), self.pages.total_pages);

        Ok(RankingEmbed::new(
            &self.users,
            &self.ranking_kind_data,
            self.author_idx,
            pages,
        ))
    }
}
