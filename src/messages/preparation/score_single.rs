#![allow(clippy::too_many_arguments)]

use crate::{
    messages::{util, AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
    util::{
        datetime::{date_to_string, how_long_ago},
        numbers::{round_and_comma, with_comma_u64},
        pp::PPProvider,
        Error,
    },
};

use rosu::models::{Beatmap, GameMode, Score, User};
use serenity::prelude::Context;

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
        ctx: &Context,
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
        let pp_provider = match PPProvider::new(&score, &map, Some(ctx)) {
            Ok(provider) => provider,
            Err(why) => {
                return Err(Error::Custom(format!(
                    "Something went wrong while creating PPProvider: {}",
                    why
                )))
            }
        };
        let grade_completion_mods =
            util::get_grade_completion_mods(&score, mode, &map, ctx.cache.clone());
        Ok(Self {
            description,
            title,
            title_url,
            author_icon,
            author_url,
            author_text,
            grade_completion_mods,
            stars: util::get_stars(&map, pp_provider.oppai()),
            score: with_comma_u64(score.score as u64),
            acc: util::get_acc(&score, mode),
            ago: how_long_ago(&score.date),
            pp: util::get_pp(&score, &pp_provider, mode),
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
