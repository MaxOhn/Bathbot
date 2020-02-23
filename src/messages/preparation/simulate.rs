#![allow(clippy::too_many_arguments)]

use crate::{
    messages::{util, AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
    util::{numbers::round, osu, pp::PPProvider, Error},
};

use rosu::models::{Beatmap, GameMode, Score};
use serenity::prelude::Context;

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
        ctx: &Context,
    ) -> Result<Self, Error> {
        if mode == GameMode::TKO || mode == GameMode::CTB {
            return Err(Error::Custom(format!(
                "Can only simulate STD and MNA scores, not {:?}",
                mode,
            )));
        }
        let title = map.to_string();
        let title_url = format!("{}b/{}", HOMEPAGE, map.beatmap_id);
        let (prev_pp, prev_combo, prev_hits, misses, pp_provider) = if let Some(s) = score.as_ref()
        {
            let pp_provider = match PPProvider::new(&s, &map, Some(ctx)) {
                Ok(provider) => provider,
                Err(why) => {
                    return Err(Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    )))
                }
            };
            let prev_pp = Some(round(pp_provider.pp()).to_string());
            let prev_combo = if mode == GameMode::STD {
                Some(s.max_combo.to_string())
            } else {
                None
            };
            let prev_hits = Some(util::get_hits(&s, mode));
            (
                prev_pp,
                prev_combo,
                prev_hits,
                Some(s.count_miss),
                Some(pp_provider),
            )
        } else {
            (None, None, None, None, None)
        };
        let mut unchoked_score = score.unwrap_or_default();
        if let Err(e) = osu::unchoke_score(&mut unchoked_score, &map, mode) {
            return Err(Error::Custom(format!(
                "Something went wrong while unchoking a score: {}",
                e
            )));
        }
        let pp_provider = if let Some(mut pp_provider) = pp_provider {
            if let Err(e) = pp_provider.recalculate(&unchoked_score, mode) {
                return Err(Error::Custom(format!(
                    "Something went wrong while recalculating PPProvider for unchoked score: {}",
                    e
                )));
            }
            pp_provider
        } else {
            match PPProvider::new(&unchoked_score, &map, Some(ctx)) {
                Ok(provider) => provider,
                Err(why) => {
                    return Err(Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    )))
                }
            }
        };
        let stars = util::get_stars(&map, pp_provider.oppai());
        let grade_completion_mods =
            util::get_grade_completion_mods(&unchoked_score, mode, &map, ctx.cache.clone());
        let pp = util::get_pp(&unchoked_score, &pp_provider, mode);
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
            removed_misses: misses,
            map_info,
            footer_url,
            footer_text,
            thumbnail,
        })
    }
}
