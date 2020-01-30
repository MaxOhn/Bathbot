#![allow(clippy::too_many_arguments)]

use crate::{
    messages::{AVATAR_URL, FLAG_URL, HOMEPAGE},
    util::numbers::{round, round_and_comma},
};

use rosu::models::{GameMode, Score, User};

pub struct WhatIfPPData {
    pub author_icon: String,
    pub author_url: String,
    pub author_text: String,
    pub title: String,
    pub thumbnail: String,
    pub description: String,
}

impl WhatIfPPData {
    pub fn new(user: Box<User>, scores: Vec<Score>, _mode: GameMode, pp: f32) -> Self {
        let author_icon = format!("{}{}.png", FLAG_URL, user.country);
        let author_url = format!("{}u/{}", HOMEPAGE, user.user_id);
        let author_text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = user.username,
            pp = round_and_comma(user.pp_raw),
            global = user.pp_rank,
            country = user.country,
            national = user.pp_country_rank
        );
        let title = format!(
            "What if {name} got a new {pp_given}pp score?",
            name = user.username,
            pp_given = pp
        );
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let pp_values: Vec<f32> = scores
            .iter()
            .map(|score| *score.pp.as_ref().unwrap())
            .collect();
        let description = if pp < pp_values[pp_values.len() - 1] {
            format!(
                "A {pp_given}pp play wouldn't even be in {name}'s top 100 plays.\nThere would not be any significant pp change.",
                pp_given = pp,
                name = user.username
            )
        } else {
            let mut actual: f32 = 0.0;
            let mut factor: f32 = 1.0;
            for score in pp_values.iter() {
                actual += score * factor;
                factor *= 0.95;
            }
            let bonus: f32 = user.pp_raw - actual;
            let mut potential: f32 = 0.0;
            let mut used: bool = false;
            let mut new_pos: i32 = -1;
            factor = 1.0;
            for i in 0..pp_values.len() - 1 {
                if !used && pp_values[i] < pp {
                    used = true;
                    potential += pp * factor;
                    factor *= 0.95;
                    new_pos = i as i32 + 1;
                }
                potential += pp_values[i] * factor;
                factor *= 0.95;
            }
            format!(
                "A {pp}pp play would be {name}'s #{num} best play.\nTheir pp would change by **{pp_change}** to **{new_pp}pp**.",
                pp = round(pp),
                name = user.username,
                num = new_pos,
                pp_change = round(potential + bonus - user.pp_raw),
                new_pp = round(potential + bonus)
            )
        };
        Self {
            author_icon,
            author_url,
            author_text,
            thumbnail,
            title,
            description,
        }
    }
}
