use crate::{
    embeds::{Author, EmbedData},
    util::{constants::AVATAR_URL, numbers::with_comma_u64},
};

use itertools::Itertools;
use rosu_v2::model::user::User;
use std::{collections::BTreeMap, fmt::Write, iter};
use twilight_embed_builder::image_source::ImageSource;

pub struct BWSEmbed {
    description: Option<String>,
    title: Option<String>,
    thumbnail: Option<ImageSource>,
    author: Option<Author>,
}

impl BWSEmbed {
    pub fn new(user: User, badges: usize, rank: Option<(u32, u32)>) -> Self {
        let stats = user.statistics.as_ref().unwrap();

        let description = match rank {
            Some((min, max)) => {
                let rank_len = max.to_string().len().max(6) + 1;
                let dist = (max - min) as usize;
                let step = 3;

                let bwss: BTreeMap<_, _> = (min..max)
                    .step_by((dist / step).max(1))
                    .take(step)
                    .chain(iter::once(max))
                    .unique()
                    .map(|rank| {
                        let bwss = (badges..=badges + 2)
                            .map(move |badges| with_comma_u64(bws(Some(rank), badges)))
                            .collect::<Vec<_>>();

                        (rank, bwss)
                    })
                    .collect();

                // Calculate the widths for each column
                let max: Vec<_> = (0..=2)
                    .map(|n| {
                        bwss.values()
                            .map(|bwss| bwss.get(n).unwrap().len())
                            .fold(0, |max, next| max.max(next))
                            .max(2)
                    })
                    .collect();

                let mut content = String::with_capacity(256);
                content.push_str("```\n");

                let _ = writeln!(
                    content,
                    " {:>rank_len$} | {:^len1$} | {:^len2$} | {:^len3$}",
                    "Badges>",
                    badges,
                    badges + 1,
                    badges + 2,
                    rank_len = rank_len,
                    len1 = max[0],
                    len2 = max[1],
                    len3 = max[2],
                );

                let _ = writeln!(
                    content,
                    "-{:->rank_len$}-+-{:-^len1$}-+-{:-^len2$}-+-{:-^len3$}-",
                    "-",
                    "-",
                    "-",
                    "-",
                    rank_len = rank_len,
                    len1 = max[0],
                    len2 = max[1],
                    len3 = max[2],
                );

                for (rank, bwss) in bwss {
                    let _ = writeln!(
                        content,
                        " {:>rank_len$} | {:^len1$} | {:^len2$} | {:^len3$}",
                        format!("#{}", rank),
                        bwss[0],
                        bwss[1],
                        bwss[2],
                        rank_len = rank_len,
                        len1 = max[0],
                        len2 = max[1],
                        len3 = max[2],
                    );
                }

                content.push_str("```");

                content
            }
            None => {
                let bws1 = with_comma_u64(bws(stats.global_rank, badges));
                let bws2 = with_comma_u64(bws(stats.global_rank, badges + 1));
                let bws3 = with_comma_u64(bws(stats.global_rank, badges + 2));
                let len1 = bws1.len().max(2);
                let len2 = bws2.len().max(2);
                let len3 = bws3.len().max(2);
                let mut content = String::with_capacity(128);
                content.push_str("```\n");

                let _ = writeln!(
                    content,
                    "Badges | {:^len1$} | {:^len2$} | {:^len3$}",
                    badges,
                    badges + 1,
                    badges + 2,
                    len1 = len1,
                    len2 = len2,
                    len3 = len3,
                );

                let _ = writeln!(
                    content,
                    "-------+-{:-^len1$}-+-{:-^len2$}-+-{:-^len3$}-",
                    "-",
                    "-",
                    "-",
                    len1 = len1,
                    len2 = len2,
                    len3 = len3,
                );

                let _ = writeln!(
                    content,
                    "   BWS | {:^len1$} | {:^len2$} | {:^len3$}",
                    bws1,
                    bws2,
                    bws3,
                    len1 = len1,
                    len2 = len2,
                    len3 = len3,
                );

                content.push_str("```");

                content
            }
        };

        let title = format!(
            "Current BWS for {} badge{}: {}",
            badges,
            if badges == 1 { "" } else { "s" },
            with_comma_u64(bws(stats.global_rank, badges))
        );

        Self {
            title: Some(title),
            description: Some(description),
            author: Some(author!(user)),
            thumbnail: Some(ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap()),
        }
    }
}

impl EmbedData for BWSEmbed {
    fn description_owned(&mut self) -> Option<String> {
        self.description.take()
    }
    fn thumbnail_owned(&mut self) -> Option<ImageSource> {
        self.thumbnail.take()
    }
    fn author_owned(&mut self) -> Option<Author> {
        self.author.take()
    }
    fn title_owned(&mut self) -> Option<String> {
        self.title.take()
    }
}

#[inline]
fn bws(rank: Option<u32>, badges: usize) -> u64 {
    let rank = rank.unwrap() as f64;
    let badges = badges as i32;

    rank.powf(0.9937_f64.powi(badges * badges)).round() as u64
}
