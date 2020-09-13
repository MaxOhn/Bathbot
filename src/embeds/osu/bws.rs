use crate::{
    embeds::{osu, Author, EmbedData},
    util::{constants::AVATAR_URL, numbers::with_comma_int},
};

use itertools::Itertools;
use rosu::models::User;
use std::{collections::BTreeMap, fmt::Write, iter};
use twilight_embed_builder::image_source::ImageSource;

#[derive(Clone)]
pub struct BWSEmbed {
    description: String,
    title: String,
    thumbnail: ImageSource,
    author: Author,
}

impl BWSEmbed {
    pub fn new(user: User, badges: usize, rank: Option<u32>) -> Self {
        let description = match rank {
            Some(rank) => {
                let (min, max) = match user.pp_rank > rank {
                    true => (rank, user.pp_rank),
                    false => (user.pp_rank, rank),
                };
                let rank_len = max.to_string().len().max(6) + 1;
                let dist = (max - min) as usize;
                let step = 3;
                let bwss: BTreeMap<_, _> = (min..max)
                    .step_by((dist / step).max(1))
                    .chain(iter::once(max))
                    .unique()
                    .map(|rank| {
                        let bwss = (badges..=badges + 2)
                            .map(move |badges| with_comma_int(bws(rank, badges)))
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
                    "-{:->rank_len$}-|-{:-^len1$}-|-{:-^len2$}-|-{:-^len3$}-",
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
                let bws1 = with_comma_int(bws(user.pp_rank, badges));
                let bws2 = with_comma_int(bws(user.pp_rank, badges + 1));
                let bws3 = with_comma_int(bws(user.pp_rank, badges + 2));
                let mut content = String::with_capacity(128);
                content.push_str("```\n");
                let _ = writeln!(
                    content,
                    "Badges | {:^len1$} | {:^len2$} | {:^len3$}",
                    badges,
                    badges + 1,
                    badges + 2,
                    len1 = bws1.len(),
                    len2 = bws2.len(),
                    len3 = bws3.len(),
                );
                let _ = writeln!(
                    content,
                    "-------+-{:-^len1$}-+-{:-^len2$}-+-{:-^len3$}",
                    "-",
                    "-",
                    "-",
                    len1 = bws1.len(),
                    len2 = bws2.len(),
                    len3 = bws3.len(),
                );
                let _ = writeln!(
                    content,
                    "   BWS | {:^len1$} | {:^len2$} | {:^len3$}",
                    bws1,
                    bws2,
                    bws3,
                    len1 = bws1.len(),
                    len2 = bws2.len(),
                    len3 = bws3.len(),
                );
                content.push_str("```");
                content
            }
        };
        let title = format!(
            "Current BWS for {} badges: {}",
            badges,
            with_comma_int(bws(user.pp_rank, badges))
        );
        Self {
            title,
            description,
            author: osu::get_user_author(&user),
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
        }
    }
}

impl EmbedData for BWSEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
}

fn bws(rank: u32, badges: usize) -> u32 {
    let rank = rank as f64;
    let badges = badges as i32;
    rank.powf(0.9937_f64.powi(badges * badges)).round() as u32
}
