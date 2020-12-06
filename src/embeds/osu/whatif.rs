use crate::{
    commands::osu::WhatIfData,
    embeds::{osu, Author, EmbedData},
    util::{
        constants::AVATAR_URL,
        numbers::{round, with_comma, with_comma_u64},
    },
};

use rosu::model::User;
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct WhatIfEmbed {
    description: String,
    title: String,
    thumbnail: ImageSource,
    author: Author,
}

impl WhatIfEmbed {
    pub fn new(user: User, pp: f32, data: WhatIfData) -> Self {
        let title = format!(
            "What if {name} got a new {pp_given}pp score?",
            name = user.username,
            pp_given = round(pp)
        );

        let description = match data {
            WhatIfData::NonTop100 => {
                format!(
                    "A {pp_given}pp play wouldn't even be in {name}'s top 100 plays.\n\
                     There would not be any significant pp change.",
                    pp_given = round(pp),
                    name = user.username
                )
            }
            WhatIfData::NoScores { rank } => {
                let mut d = format!(
                    "A {pp}pp play would be {name}'s #1 best play.\n\
                     Their pp would change by **+{pp}** to **{pp}pp**",
                    pp = with_comma(pp),
                    name = user.username,
                );

                if let Some(rank) = rank {
                    let _ = write!(
                        d,
                        "\nand they would reach rank #{}.",
                        with_comma_u64(rank.min(user.pp_rank) as u64)
                    );
                } else {
                    d.push('.');
                }

                d
            }
            WhatIfData::Top100 {
                bonus_pp,
                new_pp,
                new_pos,
                max_pp,
                rank,
            } => {
                let mut d = format!(
                    "A {pp}pp play would be {name}'s #{num} best play.\n\
                     Their pp would change by **{pp_change:+.2}** to **{new_pp}pp**",
                    pp = round(pp),
                    name = user.username,
                    num = new_pos,
                    pp_change = new_pp + bonus_pp - user.pp_raw,
                    new_pp = with_comma(new_pp + bonus_pp)
                );

                if let Some(rank) = rank {
                    let _ = write!(
                        d,
                        "\nand they would reach rank #{}.",
                        with_comma_u64(rank.min(user.pp_rank) as u64)
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
            title,
            description,
            author: osu::get_user_author(&user),
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
        }
    }
}

impl EmbedData for WhatIfEmbed {
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
