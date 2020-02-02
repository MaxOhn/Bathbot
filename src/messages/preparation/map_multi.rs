#![allow(clippy::too_many_arguments)]

use crate::{
    messages::{util, AVATAR_URL, FLAG_URL, HOMEPAGE},
    util::{
        datetime::how_long_ago,
        numbers::{round, round_and_comma, with_comma_u64},
        osu::{get_grade_emote, get_oppai},
    },
};

use rosu::models::{Beatmap, GameMode, Score, User};
use serenity::cache::CacheRwLock;

pub struct MapMultiData {
    pub author_icon: String,
    pub author_url: String,
    pub author_text: String,
    pub thumbnail: String,
    pub description: String,
}

impl MapMultiData {
    pub fn new(
        user: User,
        scores_data: Vec<(usize, Score, Beatmap)>,
        mode: GameMode,
        cache: CacheRwLock,
    ) -> Self {
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
        let thumbnail = format!("{}{}", AVATAR_URL, user.user_id);
        let mut description = String::with_capacity(512);
        for (idx, score, map) in scores_data.iter() {
            // TODO: Handle GameMode's differently
            let (oppai, max_pp) = match get_oppai(map.beatmap_id, &score, mode) {
                Ok(tuple) => tuple,
                Err(why) => panic!("Something went wrong while using oppai: {}", why),
            };
            let actual_pp = round(score.pp.unwrap_or_else(|| oppai.get_pp()));
            description.push_str(&format!(
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                 {grade} {pp} ~ ({acc}) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                idx = idx,
                title = map.title,
                version = map.version,
                base = HOMEPAGE,
                id = map.beatmap_id,
                mods = util::get_mods(&score.enabled_mods),
                stars = util::get_stars(&map, Some(oppai)),
                grade = get_grade_emote(score.grade, cache.clone()),
                pp = util::get_pp(actual_pp, max_pp),
                acc = util::get_acc(&score, mode),
                score = with_comma_u64(score.score as u64),
                combo = util::get_combo(&score, &map),
                hits = util::get_hits(&score, mode),
                ago = how_long_ago(&score.date),
            ));
            description.push('\n');
        }
        description.pop();
        Self {
            author_icon,
            author_url,
            author_text,
            thumbnail,
            description,
        }
    }
}
