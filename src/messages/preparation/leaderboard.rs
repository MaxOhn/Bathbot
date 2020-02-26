use crate::{
    scraper::ScraperScore,
    util::{
        datetime::how_long_ago,
        globals::{AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
        numbers::{round, with_comma_u64},
        osu,
        pp::PPProvider,
        Error,
    },
};

use rosu::models::{Beatmap, GameMode};
use serenity::prelude::Context;
use std::collections::HashMap;

pub struct LeaderboardData {
    pub author_icon: String,
    pub author_url: String,
    pub author_text: String,
    pub thumbnail: String,
    pub description: String,
    pub footer_text: String,
    pub footer_url: String,
}

impl LeaderboardData {
    pub fn new(
        init_name: Option<String>,
        map: Beatmap,
        scores: Vec<ScraperScore>,
        ctx: &Context,
    ) -> Result<Self, Error> {
        let mut author_text = String::with_capacity(16);
        if map.mode == GameMode::MNA {
            author_text.push_str(&format!("[{}K] ", map.diff_cs as u32));
        }
        author_text.push_str(&format!("{} [{}★]", map, round(map.stars)));
        let author_url = format!("{}b/{}", HOMEPAGE, map.beatmap_id);
        let thumbnail = format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id);
        let footer_url = format!("{}{}", AVATAR_URL, map.creator_id);
        let footer_text = format!("{:?} map by {}", map.approval_status, map.creator);
        let (description, author_icon) = if scores.is_empty() {
            ("No scores found".to_string(), String::default())
        } else {
            let author_icon = format!("{}{}", AVATAR_URL, scores.get(0).unwrap().user_id);
            let mut mod_map = HashMap::new();
            let mut description = String::with_capacity(256);
            for (i, score) in scores.into_iter().enumerate() {
                let found_author =
                    init_name.is_some() && init_name.as_ref().unwrap() == &score.username;
                let mut username = String::with_capacity(32);
                if found_author {
                    username.push_str("__");
                }
                username.push_str(&format!(
                    "[{name}](https://osu.ppy.sh/users/{id})",
                    name = score.username,
                    id = score.user_id
                ));
                if found_author {
                    username.push_str("__");
                }
                description.push_str(&format!(
                    "**{idx}.** {emote} **{name}**: {score} [ {combo} ]{mods}\n\
                     - {pp} ~ {acc}% ~ {ago}\n",
                    idx = i + 1,
                    emote = osu::grade_emote(score.grade, ctx.cache.clone()).to_string(),
                    name = username,
                    score = with_comma_u64(score.score as u64),
                    combo = get_combo(&score, &map),
                    mods = if score.enabled_mods.is_empty() {
                        String::new()
                    } else {
                        format!(" **+{}**", score.enabled_mods)
                    },
                    pp = get_pp(&mut mod_map, &score, &map, ctx)?,
                    acc = round(score.accuracy),
                    ago = how_long_ago(&score.date),
                ));
            }
            (description, author_icon)
        };
        Ok(Self {
            thumbnail,
            author_icon,
            author_text,
            author_url,
            description,
            footer_text,
            footer_url,
        })
    }
}

pub fn get_pp(
    mod_map: &mut HashMap<u32, f32>,
    score: &ScraperScore,
    map: &Beatmap,
    ctx: &Context,
) -> Result<String, Error> {
    let bits = score.enabled_mods.as_bits();
    let actual = if score.pp.is_some() {
        score.pp
    } else {
        match map.mode {
            GameMode::CTB => None,
            GameMode::STD | GameMode::TKO => Some(PPProvider::calculate_oppai_pp(score, map)?),
            GameMode::MNA => Some(PPProvider::calculate_mania_pp(score, map, ctx)?),
        }
    };
    #[allow(clippy::map_entry)]
    let max = if mod_map.contains_key(&bits) {
        mod_map.get(&bits).copied()
    } else if map.mode == GameMode::CTB {
        None
    } else {
        let max = PPProvider::calculate_max(&map, &score.enabled_mods, Some(ctx))?;
        mod_map.insert(bits, max);
        Some(max)
    };
    Ok(format!(
        "**{}**/{}PP",
        actual.map_or_else(|| "-".to_string(), |pp| round(pp).to_string()),
        max.map_or_else(|| "-".to_string(), |pp| round(pp).to_string())
    ))
}

pub fn get_combo(score: &ScraperScore, map: &Beatmap) -> String {
    let mut combo = String::from("**");
    combo.push_str(&score.max_combo.to_string());
    combo.push_str("x**/");
    match map.max_combo {
        Some(amount) => {
            combo.push_str(&amount.to_string());
            combo.push('x');
        }
        None => combo.push_str(&format!(
            " {} miss{}",
            score.count_miss,
            if score.count_miss != 1 { "es" } else { "" }
        )),
    }
    combo
}
