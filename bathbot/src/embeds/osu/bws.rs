use std::{collections::BTreeMap, fmt::Write, iter, mem};

use bathbot_macros::EmbedData;
use bathbot_model::rosu_v2::user::User;
use bathbot_util::{numbers::WithComma, AuthorBuilder, IntHasher};
use hashbrown::HashSet;

use crate::manager::redis::RedisData;

#[derive(EmbedData)]
pub struct BWSEmbed {
    description: String,
    title: String,
    thumbnail: String,
    author: AuthorBuilder,
}

struct BadgeEntry {
    count: usize,
    /// Length of `count` when stringified
    len: usize,
}

impl BWSEmbed {
    pub fn new(
        user: &RedisData<User>,
        badges_curr: usize,
        badges_min: usize,
        badges_max: usize,
        rank: Option<u32>,
    ) -> Self {
        let global_rank = user.stats().global_rank();

        let dist_badges = badges_max - badges_min;
        let step_dist = 2;

        let badges: Vec<_> = (badges_min..badges_max)
            .step_by(dist_badges / step_dist)
            .take(step_dist)
            .chain(iter::once(badges_max))
            .map(|count| BadgeEntry {
                count,
                len: WithComma::new(count).to_string().len(),
            })
            .collect();

        let yellow = "\u{001b}[1;33m";
        let reset = "\u{001b}[0m";

        let description = match rank {
            Some(rank_arg) => {
                let mut min = rank_arg;
                let mut max = global_rank;

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
                                .map(|entry| WithComma::new(bws(rank, entry.count)).to_string())
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
                            .max(badges[n].len)
                    })
                    .collect();

                let mut content = String::with_capacity(256);
                content.push_str("```ansi\n");

                let _ = writeln!(
                    content,
                    " {:>rank_len$} | {:^len1$} | {:^len2$} | {:^len3$}",
                    "Badges>",
                    badges[0].count,
                    badges[1].count,
                    badges[2].count,
                    len1 = max[0],
                    len2 = max[1],
                    len3 = max[2],
                );

                let _ = writeln!(
                    content,
                    "-{0:->rank_len$}-+-{0:-^len1$}-+-{0:-^len2$}-+-{0:-^len3$}-",
                    '-',
                    len1 = max[0],
                    len2 = max[1],
                    len3 = max[2],
                );

                for (rank, bwss) in bwss {
                    let _ = writeln!(
                        content,
                        " {:>rank_len$} | {ansi_left}{:^len1$}{reset} | {:^len2$} | {ansi_right}{:^len3$}{reset}",
                        format!("#{rank}"),
                        bwss[0],
                        bwss[1],
                        bwss[2],
                        len1 = max[0],
                        len2 = max[1],
                        len3 = max[2],
                        ansi_left = if rank == global_rank && badges_curr == badges[0].count { yellow } else { reset },
                        ansi_right = if rank == global_rank && badges_curr == badges[2].count { yellow } else { reset },
                    );
                }

                content.push_str("```");

                content
            }
            None => {
                let bws1 = WithComma::new(bws(global_rank, badges[0].count)).to_string();
                let bws2 = WithComma::new(bws(global_rank, badges[1].count)).to_string();
                let bws3 = WithComma::new(bws(global_rank, badges[2].count)).to_string();
                let len1 = bws1.len().max(2).max(badges[0].len);
                let len2 = bws2.len().max(2).max(badges[1].len);
                let len3 = bws3.len().max(2).max(badges[2].len);
                let mut content = String::with_capacity(128);
                content.push_str("```ansi\n");

                let _ = writeln!(
                    content,
                    "Badges | {:^len1$} | {:^len2$} | {:^len3$}",
                    badges[0].count, badges[1].count, badges[2].count,
                );

                let _ = writeln!(
                    content,
                    "-------+-{0:-^len1$}-+-{0:-^len2$}-+-{0:-^len3$}-",
                    '-'
                );

                let _ = writeln!(
                    content,
                    "   BWS | {ansi_left}{bws1:^len1$}{reset} | {bws2:^len2$} | {ansi_right}{bws3:^len3$}{reset}\n```",
                    ansi_left = if badges_curr == badges[0].count { yellow } else { reset },
                    ansi_right = if badges_curr == badges[2].count { yellow } else { reset },
                );

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

fn bws(rank: u32, badges: usize) -> u64 {
    let rank = rank as f64;
    let badges = badges as i32;

    rank.powf(0.9937_f64.powi(badges * badges)).round() as u64
}
