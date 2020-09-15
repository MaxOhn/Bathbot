use crate::{
    embeds::{osu, Author, EmbedData},
    util::{
        constants::AVATAR_URL,
        numbers::{round, round_and_comma},
    },
};

use rosu::models::{GameMode, Score, User};
use twilight_embed_builder::image_source::ImageSource;

pub struct WhatIfEmbed {
    description: String,
    title: String,
    thumbnail: ImageSource,
    author: Author,
}

impl WhatIfEmbed {
    pub fn new(user: User, scores: Vec<Score>, _mode: GameMode, pp: f32) -> Self {
        let title = format!(
            "What if {name} got a new {pp_given}pp score?",
            name = user.username,
            pp_given = round(pp)
        );
        let pp_values: Vec<f32> = scores
            .iter()
            .map(|score| *score.pp.as_ref().unwrap())
            .collect();
        let description = if scores.is_empty() {
            format!(
                "A {pp}pp play would be {name}'s #1 best play.\n\
                 Their pp would change by **+{pp}** to **{pp}pp**.",
                pp = round_and_comma(pp),
                name = user.username,
            )
        } else if pp < pp_values[pp_values.len() - 1] {
            format!(
                "A {pp_given}pp play wouldn't even be in {name}'s top 100 plays.\n\
                 There would not be any significant pp change.",
                pp_given = round(pp),
                name = user.username
            )
        } else {
            let mut actual: f32 = 0.0;
            let mut factor: f32 = 1.0;
            for score in pp_values.iter() {
                actual += score * factor;
                factor *= 0.95;
            }
            let bonus = user.pp_raw - actual;
            let mut potential = 0.0;
            let mut used = false;
            let mut new_pos = scores.len();
            let mut factor = 1.0;
            for (i, &pp_value) in pp_values.iter().enumerate().take(pp_values.len() - 1) {
                if !used && pp_value < pp {
                    used = true;
                    potential += pp * factor;
                    factor *= 0.95;
                    new_pos = i + 1;
                }
                potential += pp_value * factor;
                factor *= 0.95;
            }
            if !used {
                potential += pp * factor;
            }
            let mut d = format!(
                "A {pp}pp play would be {name}'s #{num} best play.\n\
                 Their pp would change by **{pp_change:+.2}** to **{new_pp}pp**.",
                pp = round(pp),
                name = user.username,
                num = new_pos,
                pp_change = potential + bonus - user.pp_raw,
                new_pp = round_and_comma(potential + bonus)
            );
            let top_pp = scores.first().and_then(|s| s.pp).unwrap_or(0.0);
            if pp > top_pp * 2.0 {
                d.push_str("\nThey'd probably also get banned :^)");
            }
            d
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
