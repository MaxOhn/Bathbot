#![allow(clippy::too_many_arguments)]

use crate::{
    messages::{util, AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
    util::{
        datetime::{date_to_string, how_long_ago},
        numbers::{round, round_and_comma, with_comma_u64},
        osu::get_oppai,
    },
};

use rosu::models::{Beatmap, GameMode, Score, User};
use serenity::cache::CacheRwLock;

pub struct SimulateData {
    pub description: Option<String>,
    pub title: String,
    pub title_url: String,
    pub stars: String,
    pub grade_completion_mods: String,
    pub acc: String,
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
        user: Box<User>,
        score: Box<Score>,
        map: Box<Beatmap>,
        personal: Vec<Score>,
        global: Vec<Score>,
        mode: GameMode,
        cache: CacheRwLock,
    ) -> Self {
        // Set description with index in personal / global top scores
        let personal_idx = personal.into_iter().position(|s| s == *score);
        let global_idx = global.into_iter().position(|s| s == *score);
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

        // Set title with (mania keys, ) artist, title, and version
        let mut title = if mode == GameMode::MNA {
            format!("{} {}", util::get_keys(&score.enabled_mods, &*map), map)
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
            global = user.pp_rank,
            country = user.country,
            national = user.pp_country_rank
        );

        // TODO: Handle GameMode's differently
        let (oppai, max_pp) = match get_oppai(map.beatmap_id, &score, mode) {
            Ok(tuple) => tuple,
            Err(why) => panic!("Something went wrong while using oppai: {}", why),
        };
        let actual_pp = round(score.pp.unwrap_or_else(|| oppai.get_pp()));
        let stars = util::get_stars(&map, Some(oppai));
        let grade_completion_mods = util::get_grade_completion_mods(&score, mode, &map, cache);
        let score_points = with_comma_u64(score.score as u64);
        let acc = util::get_acc(&score, mode);
        let pp = util::get_pp(actual_pp, round(max_pp));
        let combo = util::get_combo(&score, &map);
        let hits = util::get_hits(&score, mode);
        let map_info = util::get_map_info(&map);
        let footer_url = format!("{}{}", AVATAR_URL, map.creator_id);
        let footer_text = format!("{:?} map by {}", map.approval_status, map.creator);
        let timestamp = date_to_string(&score.date);
        let thumbnail = format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id);
        Self {
            description: description,
            title,
            title_url,
            author_icon,
            author_url,
            author_text,
            stars,
            grade_completion_mods,
            score: score_points,
            acc,
            ago: how_long_ago(&score.date),
            pp,
            combo,
            hits,
            map_info,
            footer_url,
            footer_text,
            timestamp,
            thumbnail,
        }
    }
}
