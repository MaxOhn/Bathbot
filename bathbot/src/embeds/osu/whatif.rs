use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_util::{
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils,
};

use crate::{commands::osu::WhatIfData, manager::redis::osu::CachedOsuUser, util::CachedUserExt};

#[derive(EmbedData)]
pub struct WhatIfEmbed {
    author: AuthorBuilder,
    description: String,
    thumbnail: String,
    title: String,
}

impl WhatIfEmbed {
    pub fn new(user: &CachedOsuUser, pp: f32, data: WhatIfData) -> Self {
        let (stats_pp, global_rank) = user.statistics.as_ref().map_or((0.0, 0), |stats| {
            (stats.pp.to_native(), stats.global_rank.to_native())
        });

        let username = user.username.cow_escape_markdown();
        let avatar_url = user.avatar_url.as_ref();
        let count = data.count();

        let title = if count <= 1 {
            format!(
                "What if {username} got a new {pp_given}pp score?",
                pp_given = round(pp),
            )
        } else {
            format!(
                "What if {username} got {count} new {pp_given}pp scores?",
                pp_given = round(pp),
            )
        };

        let description = match data {
            WhatIfData::NonTop100 => {
                format!(
                    "A {pp_given}pp play wouldn't even be in {username}'s top 100 plays.\n\
                    There would not be any significant pp change.",
                    pp_given = round(pp),
                )
            }
            WhatIfData::NoScores { count, rank } => {
                let mut d = if count == 1 {
                    format!(
                        "A {pp}pp play would be {username}'s #1 best play.\n\
                        Their pp would change by **+{pp}** to **{pp}pp**",
                        pp = WithComma::new(pp),
                    )
                } else {
                    format!(
                        "A {pp}pp play would be {username}'s #1 best play.\n\
                        Adding {count} of them would change their pp by **{pp:+}** to **{pp}pp**",
                        pp = WithComma::new(pp),
                    )
                };

                if let Some(rank) = rank {
                    let _ = write!(
                        d,
                        "\nand they would reach approx. rank #{} (+{}).",
                        WithComma::new(rank.min(global_rank)),
                        WithComma::new(global_rank.saturating_sub(rank)),
                    );
                } else {
                    d.push('.');
                }

                d
            }
            WhatIfData::Top100 {
                bonus_pp,
                count,
                new_pp,
                new_pos,
                max_pp,
                rank,
            } => {
                let mut d = if count == 1 {
                    format!(
                        "A {pp}pp play would be {username}'s #{new_pos} best play.\n\
                        Their pp would change by **{pp_change:+.2}** to **{new_pp}pp**",
                        pp = round(pp),
                        pp_change = (new_pp + bonus_pp - stats_pp).max(0.0),
                        new_pp = WithComma::new(new_pp + bonus_pp)
                    )
                } else {
                    format!(
                        "A {pp}pp play would be {username}'s #{new_pos} best play.\n\
                        Adding {count} of them would change their pp by **{pp_change:+.2}** to **{new_pp}pp**",
                        pp = round(pp),
                        pp_change = (new_pp + bonus_pp - stats_pp).max(0.0),
                        new_pp = WithComma::new(new_pp + bonus_pp)
                    )
                };

                if let Some(rank) = rank {
                    let _ = write!(
                        d,
                        " and they would reach approx. rank #{} (+{}).",
                        WithComma::new(rank.min(global_rank)),
                        WithComma::new(global_rank.saturating_sub(rank)),
                    );
                } else {
                    d.push('.');
                }

                if pp > max_pp * 2.0 {
                    d.push_str("\nThey'd probably also get banned :^)");
                }

                d
            }
        };

        Self {
            author: user.author_builder(),
            description,
            thumbnail: avatar_url.to_owned(),
            title,
        }
    }
}
