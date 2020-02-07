#![allow(clippy::too_many_arguments)]

use crate::{
    messages::{util, AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
    util::{
        numbers::round,
        osu::{get_oppai, unchoke_score},
    },
};

use rosu::models::{Beatmap, GameMode, Score};
use serenity::cache::CacheRwLock;

pub struct SimulateData {
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
    pub thumbnail: String,
}

impl SimulateData {
    pub fn new(score: Option<Score>, map: Beatmap, mode: GameMode, cache: CacheRwLock) -> Self {
        let title = map.to_string();
        let title_url = format!("{}b/{}", HOMEPAGE, map.beatmap_id);
        let got_score = score.is_some();

        // TODO: Handle GameMode's differently
        let mut unchoked_score = score.unwrap_or_default();
        if let Err(e) = unchoke_score(&mut unchoked_score, &map) {
            panic!("Something went wrong while unchoking a score: {}", e);
        }
        let (oppai, max_pp) = match get_oppai(map.beatmap_id, &unchoked_score, mode) {
            Ok((oppai, max_pp)) => (oppai, round(max_pp)),
            Err(why) => panic!("Something went wrong while using oppai: {}", why),
        };
        let actual_pp = if got_score {
            round(oppai.get_pp())
        } else {
            max_pp
        };
        let stars = util::get_stars(&map, Some(oppai));
        let grade_completion_mods =
            util::get_grade_completion_mods(&unchoked_score, mode, &map, cache);
        let pp = util::get_pp(actual_pp, round(max_pp));
        let (hits, combo, acc) = match mode {
            GameMode::STD => (
                util::get_hits(&unchoked_score, mode),
                util::get_combo(&unchoked_score, &map),
                util::get_acc(&unchoked_score, mode),
            ),
            GameMode::MNA => (
                String::from("{ - }"),
                String::from("**-**/-"),
                String::from("-%"),
            ),
            _ => panic!("Cannot prepare simulate data of GameMode::{:?} score", mode),
        };
        let map_info = util::get_map_info(&map);
        let footer_url = format!("{}{}", AVATAR_URL, map.creator_id);
        let footer_text = format!("{:?} map by {}", map.approval_status, map.creator);
        let thumbnail = format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id);
        Self {
            title,
            title_url,
            stars,
            grade_completion_mods,
            acc,
            pp,
            combo,
            hits,
            map_info,
            footer_url,
            footer_text,
            thumbnail,
        }
    }
}
