use std::collections::btree_map::Entry;

use bathbot_macros::pagination;
use bathbot_model::{
    rkyv_impls::UsernameWrapper, BgGameScore, RankingEntries, RankingEntry, RankingKind,
};
use eyre::{Result, WrapErr};
use rkyv::{with::DeserializeWith, Deserialize, Infallible};
use twilight_model::{channel::embed::Embed, id::Id};

use crate::{
    embeds::{EmbedData, RankingEmbed},
    manager::redis::RedisData,
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
                            let BgGameScore { discord_id, score } = scores[i];
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
                        .redis()
                        .pp_ranking(*mode, page, Some(country.as_str()))
                        .await
                        .wrap_err("failed to get ranking page")?;

                    let RankingEntries::PpU32(ref mut entries) = self.entries else { unreachable!() };

                    match ranking {
                        RedisData::Original(ranking) => {
                            let iter = ranking.ranking.into_iter().enumerate().map(|(i, user)| {
                                let entry = RankingEntry {
                                    country: Some(user.country_code.into()),
                                    name: user.username,
                                    value: user.statistics.expect("missing stats").pp.round()
                                        as u32,
                                };

                                (offset * 50 + i, entry)
                            });

                            entries.extend(iter);
                        }
                        RedisData::Archived(ranking) => {
                            let iter = ranking.ranking.iter().enumerate().map(|(i, user)| {
                                let country =
                                    user.country_code.deserialize(&mut Infallible).unwrap();

                                let name_res = UsernameWrapper::deserialize_with(
                                    &user.username,
                                    &mut Infallible,
                                );

                                let pp = user
                                    .statistics
                                    .as_ref()
                                    .map(|stats| stats.pp.round())
                                    .expect("missing stats");

                                let entry = RankingEntry {
                                    country: Some(country),
                                    name: name_res.unwrap(),
                                    value: pp as u32,
                                };

                                (offset * 50 + i, entry)
                            });

                            entries.extend(iter);
                        }
                    }
                }
                RankingKind::PpGlobal { mode } => {
                    let ranking = ctx
                        .redis()
                        .pp_ranking(*mode, page, None)
                        .await
                        .wrap_err("failed to get ranking page")?;

                    let RankingEntries::PpU32(ref mut entries) = self.entries else { unreachable!() };

                    match ranking {
                        RedisData::Original(ranking) => {
                            let iter = ranking.ranking.into_iter().enumerate().map(|(i, user)| {
                                let entry = RankingEntry {
                                    country: Some(user.country_code.into()),
                                    name: user.username,
                                    value: user.statistics.expect("missing stats").pp.round()
                                        as u32,
                                };

                                (offset * 50 + i, entry)
                            });

                            entries.extend(iter);
                        }
                        RedisData::Archived(ranking) => {
                            let iter = ranking.ranking.iter().enumerate().map(|(i, user)| {
                                let country =
                                    user.country_code.deserialize(&mut Infallible).unwrap();

                                let name_res = UsernameWrapper::deserialize_with(
                                    &user.username,
                                    &mut Infallible,
                                );

                                let pp = user
                                    .statistics
                                    .as_ref()
                                    .map(|stats| stats.pp.round())
                                    .expect("missing stats");

                                let entry = RankingEntry {
                                    country: Some(country),
                                    name: name_res.unwrap(),
                                    value: pp as u32,
                                };

                                (offset * 50 + i, entry)
                            });

                            entries.extend(iter);
                        }
                    }
                }
                RankingKind::RankedScore { mode } => {
                    let ranking = ctx
                        .osu()
                        .score_rankings(*mode)
                        .page(page)
                        .await
                        .wrap_err("failed to get ranking page")?;

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
                _ => {} // other data does not come paginated
            }
        }

        Ok(())
    }
}
