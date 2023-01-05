use std::{collections::BTreeMap, fmt::Write, iter, mem};

use bathbot_macros::EmbedData;
use bathbot_util::{numbers::WithComma, AuthorBuilder, IntHasher};
use hashbrown::HashSet;

use crate::manager::redis::{osu::User, RedisData};

#[derive(EmbedData)]
pub struct BWSEmbed {
    description: String,
    title: String,
    thumbnail: String,
    author: AuthorBuilder,
}

impl BWSEmbed {
    pub fn new(
        user: &RedisData<User>,
        badges_curr: usize,
        badges_min: usize,
        badges_max: usize,
        rank: Option<u32>,
    ) -> Self {
        let global_rank = user.peek_stats(|stats| stats.global_rank);

        let dist_badges = badges_max - badges_min;
        let step_dist = 2;

        let badges: Vec<_> = (badges_min..badges_max)
            .step_by(dist_badges / step_dist)
            .take(step_dist)
            .chain(iter::once(badges_max))
            .map(|badges| (badges, WithComma::new(badges).to_string().len()))
            .collect();

        let description = match rank {
            Some(rank_arg) => {
                let mut min = rank_arg;
                let mut max = global_rank.unwrap_or(0);

                if min > max {
                    mem::swap(&mut min, &mut max);
                }

                let rank_len = max.to_string().len().max(6) + 1;
                let dist_rank = (max - min) as usize;
                let step_rank = 3;

                let bwss: BTreeMap<_, _> = {
                    let mut values = HashSet::with_hasher(IntHasher);

                    (min..max)
                        .step_by((dist_rank / step_rank).max(1))
                        .take(step_rank)
                        .chain(iter::once(max))
                        .filter(|&n| values.insert(n))
                        .map(|rank| {
                            let bwss: Vec<_> = badges
                                .iter()
                                .map(move |(badges, _)| {
                                    WithComma::new(bws(Some(rank), *badges)).to_string()
                                })
                                .collect();

                            (rank, bwss)
                        })
                        .collect()
                };

                // Calculate the widths for each column
                let max: Vec<_> = (0..=2)
                    .map(|n| {
                        bwss.values()
                            .map(|bwss| bwss.get(n).unwrap().len())
                            .fold(0, |max, next| max.max(next))
                            .max(2)
                            .max(badges[n].1)
                    })
                    .collect();

                let mut content = String::with_capacity(256);
                content.push_str("```\n");

                let _ = writeln!(
                    content,
                    " {:>rank_len$} | {:^len1$} | {:^len2$} | {:^len3$}",
                    "Badges>",
                    badges[0].0,
                    badges[1].0,
                    badges[2].0,
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
                        format!("#{rank}"),
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
                let bws1 = WithComma::new(bws(global_rank, badges[0].0)).to_string();
                let bws2 = WithComma::new(bws(global_rank, badges[1].0)).to_string();
                let bws3 = WithComma::new(bws(global_rank, badges[2].0)).to_string();
                let len1 = bws1.len().max(2).max(badges[0].1);
                let len2 = bws2.len().max(2).max(badges[1].1);
                let len3 = bws3.len().max(2).max(badges[2].1);
                let mut content = String::with_capacity(128);
                content.push_str("```\n");

                let _ = writeln!(
                    content,
                    "Badges | {:^len1$} | {:^len2$} | {:^len3$}",
                    badges[0].0,
                    badges[1].0,
                    badges[2].0,
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
            "Current BWS for {badges_curr} badge{}: {}",
            if badges_curr == 1 { "" } else { "s" },
            WithComma::new(bws(global_rank, badges_curr))
        );

        Self {
            title,
            description,
            author: user.author_builder(),
            thumbnail: user.avatar_url().to_owned(),
        }
    }
}

fn bws(rank: Option<u32>, badges: usize) -> u64 {
    let rank = rank.unwrap_or(0) as f64;
    let badges = badges as i32;

    rank.powf(0.9937_f64.powi(badges * badges)).round() as u64
}
