#![allow(clippy::too_many_arguments)]

use crate::{
    messages::{AVATAR_URL, FLAG_URL, HOMEPAGE},
    util::numbers::{round, round_and_comma, with_comma_u64},
};

use rosu::models::{Score, User};

pub struct PPMissingData {
    pub author_icon: String,
    pub author_url: String,
    pub author_text: String,
    pub title: String,
    pub thumbnail: String,
    pub description: String,
}

impl PPMissingData {
    pub fn new(user: User, scores: Vec<Score>, pp: f32) -> Self {
        let author_icon = format!("{}{}.png", FLAG_URL, user.country);
        let author_url = format!("{}u/{}", HOMEPAGE, user.user_id);
        let author_text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = user.username,
            pp = round_and_comma(user.pp_raw),
            global = with_comma_u64(user.pp_rank as u64),
            country = user.country,
            national = user.pp_country_rank
        );
        let title = format!(
            "What score is missing for {name} to reach {pp_given}pp?",
            name = user.username,
            pp_given = pp
        );
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let description = if user.pp_raw > pp {
            format!(
                "{name} already has {pp_raw}pp which is more than {pp_given}pp.\n\
                 No more scores are required.",
                name = user.username,
                pp_raw = round_and_comma(user.pp_raw),
                pp_given = pp
            )
        } else {
            let pp_values: Vec<f32> = scores
                .iter()
                .map(|score| *score.pp.as_ref().unwrap())
                .collect();
            let size: usize = pp_values.len();
            let mut idx: usize = size - 1;
            let mut factor: f32 = 0.95_f32.powi(idx as i32);
            let mut top: f32 = user.pp_raw;
            let mut bot: f32 = 0.0;
            let mut current: f32 = pp_values[idx];
            while top + bot < pp {
                top -= current * factor;
                if idx == 0 {
                    break;
                }
                current = pp_values[idx - 1];
                bot += current * factor;
                factor /= 0.95;
                idx -= 1;
            }
            let mut required: f32 = pp - top - bot;
            if top + bot >= pp {
                factor *= 0.95;
                required = (required + factor * pp_values[idx]) / factor;
                idx += 1;
            }
            idx += 1;
            if size < 100 {
                required -= pp_values[size - 1] * 0.95_f32.powi(size as i32 - 1);
            }
            format!(
                "To reach {pp}pp with one additional score, {user} needs to perform \
                 a **{required}pp** score which would be the top #{idx}",
                pp = round(pp),
                user = user.username,
                required = round(required),
                idx = idx
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
