use std::collections::{btree_map::Entry, BTreeMap};

use command_macros::pagination;
use eyre::Report;
use rosu_v2::prelude::Rankings;
use twilight_model::{channel::embed::Embed, id::Id};

use crate::{
    commands::osu::UserValue,
    embeds::{EmbedData, RankingEmbed, RankingEntry, RankingKindData},
    BotResult, Context,
};

use super::Pages;

type Users = BTreeMap<usize, RankingEntry>;

#[pagination(per_page = 20, total = "total")]
pub struct RankingPagination {
    users: Users,
    total: usize,
    author_idx: Option<usize>,
    ranking_kind_data: RankingKindData,
}

impl RankingPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> BotResult<Embed> {
        let idx = pages.index.saturating_sub(1);
        let mut page = ((idx - idx % 50) + 50) / 50;
        page += self.users.contains_key(&idx) as usize;

        self.assure_present_users(ctx, pages, page).await?;

        // Handle edge cases like idx=140;total=151 where two pages have to be requested at once
        self.assure_present_users(ctx, pages, page + 1).await?;

        let embed = RankingEmbed::new(&self.users, &self.ranking_kind_data, self.author_idx, pages);

        Ok(embed.build())
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

    async fn assure_present_users(
        &mut self,
        ctx: &Context,
        pages: &Pages,
        page: usize,
    ) -> BotResult<()> {
        let count = self
            .users
            .range(pages.index..pages.index + pages.per_page)
            .count();

        if count < pages.per_page && count < self.total - pages.index {
            let offset = page - 1;
            let page = page as u32;
            let kind = &self.ranking_kind_data;

            match kind {
                RankingKindData::BgScores { scores, .. } => {
                    #[allow(clippy::needless_range_loop)]
                    for i in pages.index..(pages.index + pages.per_page).min(self.total) {
                        if let Entry::Vacant(entry) = self.users.entry(i) {
                            let (id, score) = scores[i];
                            let id = Id::new(id);

                            let name = match ctx.psql().get_user_osu(id).await {
                                Ok(Some(osu)) => osu.into_username(),
                                Ok(None) => ctx
                                    .cache
                                    .user(id, |user| user.name.clone())
                                    .unwrap_or_else(|_| "Unknown user".to_owned())
                                    .into(),
                                Err(err) => {
                                    let report =
                                        Report::new(err).wrap_err("failed to get osu user");
                                    warn!("{report:?}");

                                    ctx.cache
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
                    let ranking = ctx
                        .osu()
                        .performance_rankings(*mode)
                        .country(country.as_str())
                        .page(page)
                        .await?;

                    self.extend_from_ranking(ranking, offset);
                }
                RankingKindData::PpGlobal { mode } => {
                    let ranking = ctx.osu().performance_rankings(*mode).page(page).await?;

                    self.extend_from_ranking(ranking, offset);
                }
                RankingKindData::RankedScore { mode } => {
                    let ranking = ctx.osu().score_rankings(*mode).page(page).await?;
                    self.extend_from_ranking(ranking, offset);
                }
                _ => {} // other data does not come paginated
            }
        }

        Ok(())
    }
}
