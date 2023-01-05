use std::collections::btree_map::Entry;

use bathbot_macros::pagination;
use bathbot_model::{RankingEntries, RankingEntry, RankingKind};
use bathbot_psql::model::games::DbBgGameScore;
use eyre::{Result, WrapErr};
use rosu_v2::prelude::Rankings;
use twilight_model::{channel::embed::Embed, id::Id};

use crate::{
    embeds::{EmbedData, RankingEmbed},
    Context,
};

use super::Pages;

#[pagination(per_page = 20, total = "total")]
pub struct RankingPagination {
    entries: RankingEntries,
    total: usize,
    author_idx: Option<usize>,
    kind: RankingKind,
}

impl RankingPagination {
    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let idx = pages.index.saturating_sub(1);
        let mut page = ((idx - idx % 50) + 50) / 50;
        page += self.entries.contains_key(idx) as usize;

        self.assure_present_users(ctx, pages, page).await?;

        // Handle edge cases like idx=140;total=151 where two pages have to be requested at once
        self.assure_present_users(ctx, pages, page + 1).await?;

        let embed = RankingEmbed::new(&self.entries, &self.kind, self.author_idx, pages);

        Ok(embed.build())
    }

    fn extend_from_ranking(&mut self, ranking: Rankings, offset: usize) {
        match self.kind {
            RankingKind::PpCountry { .. } => {
                let RankingEntries::PpU32(ref mut entries) = self.entries else { unreachable!() };

                let iter = ranking.ranking.into_iter().enumerate().map(|(i, user)| {
                    let entry = RankingEntry {
                        country: Some(user.country_code.into()),
                        name: user.username,
                        value: user.statistics.expect("missing stats").pp.round() as u32,
                    };

                    (offset * 50 + i, entry)
                });

                entries.extend(iter);
            }
            RankingKind::PpGlobal { .. } => {
                let RankingEntries::PpU32(ref mut entries) = self.entries else { unreachable!() };

                let iter = ranking.ranking.into_iter().enumerate().map(|(i, user)| {
                    let entry = RankingEntry {
                        country: Some(user.country_code.into()),
                        name: user.username,
                        value: user.statistics.expect("missing stats").pp.round() as u32,
                    };

                    (offset * 50 + i, entry)
                });

                entries.extend(iter);
            }
            RankingKind::RankedScore { .. } => {
                let RankingEntries::Amount(ref mut entries) = self.entries else { unreachable!() };

                let iter = ranking.ranking.into_iter().enumerate().map(|(i, user)| {
                    let entry = RankingEntry {
                        country: Some(user.country_code.into()),
                        name: user.username,
                        value: user.statistics.expect("missing stats").ranked_score,
                    };

                    (offset * 50 + i, entry)
                });

                entries.extend(iter);
            }
            _ => unreachable!(),
        }
    }

    async fn assure_present_users(
        &mut self,
        ctx: &Context,
        pages: &Pages,
        page: usize,
    ) -> Result<()> {
        let range = pages.index..pages.index + pages.per_page;
        let count = self.entries.entry_count(range);

        if count < pages.per_page && count < self.total - pages.index {
            let offset = page - 1;
            let page = page as u32;
            let kind = &self.kind;

            match kind {
                RankingKind::BgScores { scores, .. } => {
                    let RankingEntries::Amount(ref mut entries) = self.entries else { unreachable!() };

                    #[allow(clippy::needless_range_loop)]
                    for i in pages.index..(pages.index + pages.per_page).min(self.total) {
                        if let Entry::Vacant(entry) = entries.entry(i) {
                            let DbBgGameScore { discord_id, score } = scores[i];
                            let id = Id::new(discord_id as u64);

                            let name = match ctx.user_config().osu_name(id).await {
                                Ok(Some(name)) => name,
                                Ok(None) => ctx
                                    .cache
                                    .user(id, |user| user.name.as_str().into())
                                    .unwrap_or_else(|_| "Unknown user".into()),
                                Err(err) => {
                                    warn!("{:?}", err.wrap_err("failed to get osu user"));

                                    ctx.cache
                                        .user(id, |user| user.name.as_str().into())
                                        .unwrap_or_else(|_| "Unknown user".into())
                                }
                            };

                            entry.insert(RankingEntry {
                                country: None,
                                name,
                                value: score as u64,
                            });
                        }
                    }
                }
                RankingKind::PpCountry {
                    mode,
                    country_code: country,
                    ..
                } => {
                    let ranking = ctx
                        .osu()
                        .performance_rankings(*mode)
                        .country(country.as_str())
                        .page(page)
                        .await
                        .wrap_err("failed to get ranking page")?;

                    self.extend_from_ranking(ranking, offset);
                }
                RankingKind::PpGlobal { mode } => {
                    let ranking = ctx
                        .osu()
                        .performance_rankings(*mode)
                        .page(page)
                        .await
                        .wrap_err("failed to get ranking page")?;

                    self.extend_from_ranking(ranking, offset);
                }
                RankingKind::RankedScore { mode } => {
                    let ranking = ctx
                        .osu()
                        .score_rankings(*mode)
                        .page(page)
                        .await
                        .wrap_err("failed to get ranking page")?;

                    self.extend_from_ranking(ranking, offset);
                }
                _ => {} // other data does not come paginated
            }
        }

        Ok(())
    }
}
