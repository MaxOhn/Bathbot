use std::{
    collections::{btree_map::Range, BTreeMap},
    fmt::{Display, Formatter, Result as FmtResult, Write},
};

use bathbot_model::{EmbedHeader, RankingEntries, RankingEntry, RankingKind};
use bathbot_util::{
    numbers::{round, WithComma},
    EmbedBuilder, FooterBuilder,
};
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

use crate::{embeds::EmbedData, pagination::Pages};

pub struct RankingEmbed {
    description: String,
    footer: FooterBuilder,
    header: EmbedHeader,
}

impl RankingEmbed {
    pub fn new(
        entries: &RankingEntries,
        kind: &RankingKind,
        author_idx: Option<usize>,
        pages: &Pages,
    ) -> Self {
        let page = pages.curr_page();
        let pages = pages.last_page();
        let idx = (page - 1) * 20;
        let mut buf = String::new();
        let description = String::with_capacity(1024);
        let footer = kind.footer(page, pages, author_idx);
        let header = kind.embed_header();

        match entries {
            RankingEntries::Accuracy(entries) => Self::finalize::<_, Accuracy<'_>>(
                &mut buf,
                description,
                entries,
                idx,
                footer,
                header,
            ),
            RankingEntries::Amount(entries) => {
                Self::finalize::<_, Amount<'_>>(&mut buf, description, entries, idx, footer, header)
            }
            RankingEntries::AmountWithNegative(entries) => {
                Self::finalize::<_, AmountWithNegative<'_>>(
                    &mut buf,
                    description,
                    entries,
                    idx,
                    footer,
                    header,
                )
            }
            RankingEntries::Date(entries) => {
                Self::finalize::<_, Date<'_>>(&mut buf, description, entries, idx, footer, header)
            }
            RankingEntries::Float(entries) => {
                Self::finalize::<_, Float<'_>>(&mut buf, description, entries, idx, footer, header)
            }
            RankingEntries::Playtime(entries) => Self::finalize::<_, Playtime<'_>>(
                &mut buf,
                description,
                entries,
                idx,
                footer,
                header,
            ),
            RankingEntries::PpF32(entries) => {
                Self::finalize::<_, PpF32<'_>>(&mut buf, description, entries, idx, footer, header)
            }
            RankingEntries::PpU32(entries) => {
                Self::finalize::<_, PpU32<'_>>(&mut buf, description, entries, idx, footer, header)
            }
            RankingEntries::Rank(entries) => {
                Self::finalize::<_, Rank<'_>>(&mut buf, description, entries, idx, footer, header)
            }
        }
    }

    fn finalize<'v, V, F>(
        buf: &mut String,
        mut description: String,
        entries: &'v BTreeMap<usize, RankingEntry<V>>,
        idx: usize,
        footer: FooterBuilder,
        header: EmbedHeader,
    ) -> Self
    where
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

        Self {
            description,
            footer,
            header,
        }
    }
}

impl EmbedData for RankingEmbed {
    fn build(self) -> Embed {
        let builder = EmbedBuilder::new()
            .description(self.description)
            .footer(self.footer);

        match self.header {
            EmbedHeader::Author(author) => builder.author(author).build(),
            EmbedHeader::Title { text, url } => builder.title(text).url(url).build(),
        }
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
                fn from(value: &'i $ty) -> Self {
                    Self { inner: value }
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
        write!(f, "{}", AmountWithNegative::from(&(*self.inner as i64)))
    }
}

impl Display for AmountWithNegative<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.inner.abs() < 1_000_000_000 {
            write!(f, "{}", WithComma::new(*self.inner))
        } else {
            let score = (self.inner / 10_000_000) as f32 / 100.0;

            write!(f, "{score:.2} bn")
        }
    }
}

impl Display for Date<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.inner.date())
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
