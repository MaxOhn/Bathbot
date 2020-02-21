#![allow(clippy::too_many_arguments)]

use crate::{
    messages::{util, AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
    util::{
        datetime::{date_to_string, how_long_ago},
        numbers::{round, round_and_comma, with_comma_u64},
        osu, Error,
    },
};

use rosu::models::{Beatmap, GameMode, Score, User};
use serenity::cache::CacheRwLock;

pub struct ScoreSingleData {
    pub description: Option<String>,
    pub title: String,
    pub title_url: String,
    pub author_icon: String,
    pub author_url: String,
    pub author_text: String,
    pub stars: String,
    pub grade_completion_mods: String,
    pub score: String,
    pub acc: String,
    pub ago: String,
    pub pp: String,
    pub combo: String,
    pub hits: String,
    pub map_info: String,
    pub footer_url: String,
    pub footer_text: String,
    pub timestamp: String,
    pub thumbnail: String,
}

impl ScoreSingleData {
    pub fn new(
        user: User,
        score: Score,
        map: Beatmap,
        personal: Vec<Score>,
        global: Vec<Score>,
        mode: GameMode,
        cache: CacheRwLock,
    ) -> Result<Self, Error> {
        let personal_idx = personal.into_iter().position(|s| s == score);
        let global_idx = global.into_iter().position(|s| s == score);
        let description = if personal_idx.is_some() || global_idx.is_some() {
            let mut description = String::from("__**");
            if let Some(idx) = personal_idx {
                description.push_str("Personal Best #");
                description.push_str(&(idx + 1).to_string());
                if global_idx.is_some() {
                    description.push_str(" and ");
                }
            }
            if let Some(idx) = global_idx {
                description.push_str("Global Top #");
                description.push_str(&(idx + 1).to_string());
            }
            description.push_str("**__");
            Some(description)
        } else {
            None
        };
        let title = if mode == GameMode::MNA {
            format!("{} {}", util::get_keys(&score.enabled_mods, &map), map)
        } else {
            map.to_string()
        };
        let title_url = format!("{}b/{}", HOMEPAGE, map.beatmap_id);
        let author_icon = format!("{}{}", AVATAR_URL, user.user_id);
        let author_url = format!("{}u/{}", HOMEPAGE, user.user_id);
        let author_text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = user.username,
            pp = round_and_comma(user.pp_raw),
            global = with_comma_u64(user.pp_rank as u64),
            country = user.country,
            national = user.pp_country_rank
        );
        // TODO: Handle GameMode's differently
        let (oppai, max_pp) = match osu::oppai_max_pp(map.beatmap_id, &score, mode) {
            Ok(tuple) => tuple,
            Err(why) => {
                return Err(Error::Custom(format!(
                    "Something went wrong while using oppai: {}",
                    why
                )))
            }
        };
        let actual_pp = round(score.pp.unwrap_or_else(|| oppai.get_pp()));
        let grade_completion_mods = util::get_grade_completion_mods(&score, mode, &map, cache);
        Ok(Self {
            description,
            title,
            title_url,
            author_icon,
            author_url,
            author_text,
            grade_completion_mods,
            stars: util::get_stars(&map, Some(oppai)),
            score: with_comma_u64(score.score as u64),
            acc: util::get_acc(&score, mode),
            ago: how_long_ago(&score.date),
            pp: util::get_pp(actual_pp, round(max_pp)),
            combo: util::get_combo(&score, &map),
            hits: util::get_hits(&score, mode),
            map_info: util::get_map_info(&map),
            footer_url: format!("{}{}", AVATAR_URL, map.creator_id),
            footer_text: format!("{:?} map by {}", map.approval_status, map.creator),
            timestamp: date_to_string(&score.date),
            thumbnail: format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id),
        })
    }
}
