use std::{
    collections::{
        btree_map::{Entry, Range},
        BTreeMap,
    },
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{BgGameScore, EmbedHeader, RankingEntries, RankingEntry, RankingKind};
use bathbot_util::{
    numbers::{round, WithComma},
    EmbedBuilder,
};
use eyre::{Result, WrapErr};
use futures::future::BoxFuture;
use time::OffsetDateTime;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    core::{Context, ContextExt},
    manager::redis::RedisData,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct RankingPagination {
    #[pagination(per_page = 20, len = "total")]
    entries: RankingEntries,
    total: usize,
    author_idx: Option<usize>,
    kind: RankingKind,
    defer: bool,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for RankingPagination {
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        Box::pin(self.async_build_page(ctx))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(
            ctx,
            component,
            self.msg_owner,
            self.defer(),
            &mut self.pages,
        )
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(ctx, modal, self.msg_owner, self.defer(), &mut self.pages)
    }
}

impl RankingPagination {
    fn defer(&self) -> bool {
        matches!(
            self.kind,
            RankingKind::BgScores { .. }
                | RankingKind::PpCountry { .. }
                | RankingKind::PpGlobal { .. }
                | RankingKind::RankedScore { .. }
        )
    }

    async fn async_build_page(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let idx = self.pages.index().saturating_sub(1);
        let mut page = ((idx - idx % 50) + 50) / 50;
        page += self.entries.contains_key(idx) as usize;

        self.assure_present_users(ctx.cloned(), page).await?;

        // Handle edge cases like idx=140;total=151 where two pages have to be requested
        // at once
        self.assure_present_users(ctx.cloned(), page + 1).await?;

        let idx = self.pages.index();

        let mut buf = String::new();
        let mut description = String::with_capacity(1024);

        match self.entries {
            RankingEntries::Accuracy(ref entries) => {
                Self::finalize::<_, Accuracy<'_>>(&mut buf, &mut description, entries, idx)
            }
            RankingEntries::Amount(ref entries) => {
                Self::finalize::<_, Amount<'_>>(&mut buf, &mut description, entries, idx)
            }
            RankingEntries::AmountWithNegative(ref entries) => {
                Self::finalize::<_, AmountWithNegative<'_>>(
                    &mut buf,
                    &mut description,
                    entries,
                    idx,
                )
            }
            RankingEntries::Date(ref entries) => {
                Self::finalize::<_, Date<'_>>(&mut buf, &mut description, entries, idx)
            }
            RankingEntries::Float(ref entries) => {
                Self::finalize::<_, Float<'_>>(&mut buf, &mut description, entries, idx)
            }
            RankingEntries::Playtime(ref entries) => {
                Self::finalize::<_, Playtime<'_>>(&mut buf, &mut description, entries, idx)
            }
            RankingEntries::PpF32(ref entries) => {
                Self::finalize::<_, PpF32<'_>>(&mut buf, &mut description, entries, idx)
            }
            RankingEntries::PpU32(ref entries) => {
                Self::finalize::<_, PpU32<'_>>(&mut buf, &mut description, entries, idx)
            }
            RankingEntries::Rank(ref entries) => {
                Self::finalize::<_, Rank<'_>>(&mut buf, &mut description, entries, idx)
            }
        };

        let page = self.pages.curr_page();
        let pages = self.pages.last_page();
        let footer = self.kind.footer(page, pages, self.author_idx);

        let mut builder = EmbedBuilder::new().description(description).footer(footer);

        builder = match self.kind.embed_header() {
            EmbedHeader::Author(author) => builder.author(author),
            EmbedHeader::Title { text, url } => builder.title(text).url(url),
        };

        Ok(BuildPage::new(builder, self.defer))
    }

    fn finalize<'v, V, F>(
        buf: &mut String,
        description: &mut String,
        entries: &'v BTreeMap<usize, RankingEntry<V>>,
        idx: usize,
    ) where
        F: From<&'v V> + Display,
        V: 'v,
    {
        let left_lengths = Lengths::new::<V, F>(buf, entries.range(idx..idx + 10));
        let right_lengths = Lengths::new::<V, F>(buf, entries.range(idx + 10..idx + 20));

        // Ensuring the right side has ten elements for the zip
        let user_iter = entries
            .range(idx..idx + 10)
            .zip((10..20).map(|i| entries.get(&(idx + i))));

        for ((i, left_entry), right) in user_iter {
            let idx = i + 1;

            buf.clear();
            let _ = write!(buf, "{}", F::from(&left_entry.value));

            let _ = write!(
                description,
                "`#{idx:<idx_len$}`{country}`{name:<name_len$}` `{buf:>value_len$}`",
                idx_len = left_lengths.idx,
                country = CountryFormatter::new(left_entry),
                name = left_entry.name,
                name_len = left_lengths.name,
                value_len = left_lengths.value,
            );

            if let Some(right_entry) = right {
                buf.clear();
                let _ = write!(buf, "{}", F::from(&right_entry.value));

                let _ = write!(
                    description,
                    "|`#{idx:<idx_len$}`{country}`{name:<name_len$}` `{buf:>value_len$}`",
                    idx = idx + 10,
                    idx_len = right_lengths.idx,
                    country = CountryFormatter::new(right_entry),
                    name = right_entry.name,
                    name_len = right_lengths.name,
                    value_len = right_lengths.value,
                );
            }

            description.push('\n');
        }
    }

    async fn assure_present_users(&mut self, ctx: Arc<Context>, page: usize) -> Result<()> {
        let pages = &self.pages;
        let range = pages.index()..pages.index() + pages.per_page();
        let count = self.entries.entry_count(range);

        if count < pages.per_page() && count < self.total - pages.index() {
            let offset = page - 1;
            let page = page as u32;
            let kind = &self.kind;

            match kind {
                RankingKind::BgScores { scores, .. } => {
                    let RankingEntries::Amount(ref mut entries) = self.entries else {
                        unreachable!()
                    };

                    // not necessary but less ugly than the iterator
                    #[allow(clippy::needless_range_loop)]
                    for i in pages.index()..(pages.index() + pages.per_page()).min(self.total) {
                        if let Entry::Vacant(entry) = entries.entry(i) {
                            let BgGameScore { discord_id, score } = scores[i];
                            let id = Id::new(discord_id as u64);

                            let mut name_opt = match ctx.user_config().osu_name(id).await {
                                Ok(Some(name)) => Some(name),
                                Ok(None) => None,
                                Err(err) => {
                                    warn!(?err, "Failed to get osu user");

                                    None
                                }
                            };

                            name_opt = match name_opt {
                                Some(name) => Some(name),
                                None => match ctx.cache.user(id).await {
                                    Ok(Some(user)) => Some(user.name.as_ref().into()),
                                    Ok(None) => None,
                                    Err(err) => {
                                        warn!("{err:?}");

                                        None
                                    }
                                },
                            };

                            entry.insert(RankingEntry {
                                country: None,
                                name: name_opt.unwrap_or_else(|| "Unknown user".into()),
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
                        .wrap_err("Failed to get ranking page")?;

                    let RankingEntries::PpU32(ref mut entries) = self.entries else {
                        unreachable!()
                    };

                    match ranking {
                        RedisData::Original(ranking) => {
                            let iter = ranking.ranking.into_iter().enumerate().map(|(i, user)| {
                                let entry = RankingEntry {
                                    country: Some(user.country_code),
                                    name: user.username,
                                    value: user.statistics.expect("missing stats").pp.round()
                                        as u32,
                                };

                                (offset * 50 + i, entry)
                            });

                            entries.extend(iter);
                        }
                        RedisData::Archive(ranking) => {
                            let iter = ranking.ranking.iter().enumerate().map(|(i, user)| {
                                let country = user.country_code.as_str().into();

                                let pp = user
                                    .statistics
                                    .as_ref()
                                    .map(|stats| stats.pp.round())
                                    .expect("missing stats");

                                let entry = RankingEntry {
                                    country: Some(country),
                                    name: user.username.as_str().into(),
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

                    let RankingEntries::PpU32(ref mut entries) = self.entries else {
                        unreachable!()
                    };

                    match ranking {
                        RedisData::Original(ranking) => {
                            let iter = ranking.ranking.into_iter().enumerate().map(|(i, user)| {
                                let entry = RankingEntry {
                                    country: Some(user.country_code),
                                    name: user.username,
                                    value: user.statistics.expect("missing stats").pp.round()
                                        as u32,
                                };

                                (offset * 50 + i, entry)
                            });

                            entries.extend(iter);
                        }
                        RedisData::Archive(ranking) => {
                            let iter = ranking.ranking.iter().enumerate().map(|(i, user)| {
                                let country = user.country_code.as_str().into();

                                let pp = user
                                    .statistics
                                    .as_ref()
                                    .map(|stats| stats.pp.round())
                                    .expect("missing stats");

                                let entry = RankingEntry {
                                    country: Some(country),
                                    name: user.username.as_str().into(),
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
                        .wrap_err("Failed to get ranking page")?;

                    let RankingEntries::Amount(ref mut entries) = self.entries else {
                        unreachable!()
                    };

                    let iter = ranking.ranking.into_iter().enumerate().map(|(i, user)| {
                        let entry = RankingEntry {
                            country: Some(user.country_code),
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

struct Lengths {
    idx: usize,
    name: usize,
    value: usize,
}

impl Lengths {
    fn new<'v, V, F>(buf: &mut String, iter: Range<'v, usize, RankingEntry<V>>) -> Self
    where
        F: From<&'v V> + Display,
        V: 'v,
    {
        let mut idx_len = 0;
        let mut name_len = 0;
        let mut value_len = 0;

        for (i, entry) in iter {
            let mut idx = i + 1;
            let mut len = 0;

            while idx > 0 {
                len += 1;
                idx /= 10;
            }

            idx_len = idx_len.max(len);
            name_len = name_len.max(entry.name.chars().count());

            buf.clear();
            let _ = write!(buf, "{}", F::from(&entry.value));
            value_len = value_len.max(buf.len());
        }

        Lengths {
            idx: idx_len,
            name: name_len,
            value: value_len,
        }
    }
}

struct CountryFormatter<'e, V> {
    entry: &'e RankingEntry<V>,
}

impl<'e, V> CountryFormatter<'e, V> {
    fn new(entry: &'e RankingEntry<V>) -> Self {
        Self { entry }
    }
}

impl<V> Display for CountryFormatter<'_, V> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if let Some(ref country) = self.entry.country {
            write!(f, ":flag_{}:", country.to_ascii_lowercase())
        } else {
            f.write_str(" ")
        }
    }
}

macro_rules! formatter {
    ( $( $name:ident<$ty:ident> ,)* ) => {
        $(
            struct $name<'i> {
                inner: &'i $ty,
            }

            impl<'i> From<&'i $ty> for $name<'i> {
                #[inline]
                fn from(inner: &'i $ty) -> Self {
                    Self { inner }
                }
            }
        )*
    };
}

formatter! {
    Accuracy<f32>,
    Amount<u64>,
    AmountWithNegative<i64>,
    Date<OffsetDateTime>,
    Float<f32>,
    Playtime<u32>,
    PpF32<f32>,
    PpU32<u32>,
    Rank<u32>,
}

impl Display for Accuracy<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:.2}%", self.inner)
    }
}

impl Display for Amount<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&AmountWithNegative::from(&(*self.inner as i64)), f)
    }
}

impl Display for AmountWithNegative<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.inner.abs() < 1_000_000_000 {
            Display::fmt(&WithComma::new(*self.inner), f)
        } else {
            let score = (self.inner / 10_000_000) as f32 / 100.0;

            write!(f, "{score:.2} bn")
        }
    }
}

impl Display for Date<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.inner.date(), f)
    }
}

impl Display for Float<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:.2}", self.inner)
    }
}

impl Display for Playtime<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{} hrs", WithComma::new(self.inner / 60 / 60))
    }
}

impl Display for PpF32<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}pp", WithComma::new(round(*self.inner)))
    }
}

impl Display for PpU32<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}pp", WithComma::new(*self.inner))
    }
}

impl Display for Rank<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "#{}", WithComma::new(*self.inner))
    }
}
