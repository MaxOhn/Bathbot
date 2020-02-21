#![allow(clippy::too_many_arguments)]

use crate::{
    messages::{util, AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
    util::{numbers::round, osu, Error},
};

use rosu::models::{Beatmap, GameMode, Score};
use serenity::cache::CacheRwLock;

pub struct SimulateData {
    pub title: String,
    pub title_url: String,
    pub stars: String,
    pub grade_completion_mods: String,
    pub acc: String,
    pub prev_pp: Option<String>,
    pub pp: String,
    pub prev_combo: Option<String>,
    pub combo: String,
    pub prev_hits: Option<String>,
    pub hits: String,
    pub removed_misses: Option<u32>,
    pub map_info: String,
    pub footer_url: String,
    pub footer_text: String,
    pub thumbnail: String,
}

impl SimulateData {
    pub fn new(
        score: Option<Score>,
        map: Beatmap,
        mode: GameMode,
        cache: CacheRwLock,
    ) -> Result<Self, Error> {
        let title = map.to_string();
        let title_url = format!("{}b/{}", HOMEPAGE, map.beatmap_id);
        let got_score = score.is_some();
        let (prev_pp, prev_combo, prev_hits, removed_misses) = if let Some(score) = score.as_ref() {
            // TODO: Handle GameMode's differently
            let pp = if let Some(pp) = score.pp {
                pp
            } else {
                osu::pp(map.beatmap_id, score, mode, None)?
            };
            let prev_pp = Some(round(pp).to_string());
            let prev_combo = if mode == GameMode::STD {
                Some(score.max_combo.to_string())
            } else {
                None
            };
            let prev_hits = Some(util::get_hits(&score, mode));
            (prev_pp, prev_combo, prev_hits, Some(score.count_miss))
        } else {
            (None, None, None, None)
        };

        // TODO: Handle GameMode's differently
        let mut unchoked_score = score.unwrap_or_default();
        if let Err(e) = osu::unchoke_score(&mut unchoked_score, &map) {
            return Err(Error::Custom(format!(
                "Something went wrong while unchoking a score: {}",
                e
            )));
        }
        let (oppai, max_pp) = match osu::oppai_max_pp(map.beatmap_id, &unchoked_score, mode) {
            Ok((oppai, max_pp)) => (oppai, round(max_pp)),
            Err(why) => {
                return Err(Error::Custom(format!(
                    "Something went wrong while using oppai: {}",
                    why
                )))
            }
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
            _ => {
                return Err(Error::Custom(format!(
                    "Cannot prepare simulate data of GameMode::{:?} score",
                    mode
                )))
            }
        };
        let map_info = util::get_map_info(&map);
        let footer_url = format!("{}{}", AVATAR_URL, map.creator_id);
        let footer_text = format!("{:?} map by {}", map.approval_status, map.creator);
        let thumbnail = format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id);
        Ok(Self {
            title,
            title_url,
            stars,
            grade_completion_mods,
            acc,
            prev_pp,
            pp,
            prev_combo,
            combo,
            prev_hits,
            hits,
            removed_misses,
            map_info,
            footer_url,
            footer_text,
            thumbnail,
        })
    }
}
