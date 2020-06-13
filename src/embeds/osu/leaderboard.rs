use crate::{
    embeds::{Author, EmbedData, Footer},
    scraper::ScraperScore,
    util::{
        datetime::how_long_ago,
        discord::CacheData,
        globals::{AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
        numbers::{round, with_comma_u64},
        osu,
        pp::PPProvider,
    },
    Error,
};

use rosu::models::{Beatmap, GameMode};
use serenity::prelude::{RwLock, TypeMap};
use std::{collections::HashMap, fmt::Write};

#[derive(Clone)]
pub struct LeaderboardEmbed {
    description: String,
    thumbnail: String,
    author: Author,
    footer: Footer,
}

impl LeaderboardEmbed {
    pub async fn new<'i, S, D>(
        init_name: &Option<&str>,
        map: &Beatmap,
        scores: Option<S>,
        author_icon: &Option<String>,
        idx: usize,
        cache_data: D,
    ) -> Result<Self, Error>
    where
        S: Iterator<Item = &'i ScraperScore>,
        D: CacheData,
    {
        let mut author_text = String::with_capacity(32);
        if map.mode == GameMode::MNA {
            let _ = write!(author_text, "[{}K] ", map.diff_cs as u32);
        }
        let _ = write!(author_text, "{} [{}★]", map, round(map.stars));
        let footer_url = format!("{}{}", AVATAR_URL, map.creator_id);
        let footer_text = format!("{:?} map by {}", map.approval_status, map.creator);
        let description = if let Some(scores) = scores {
            let mut mod_map = HashMap::new();
            let mut description = String::with_capacity(256);
            let author_name = init_name.map_or_else(String::new, |n| n.to_lowercase());
            for (i, score) in scores.enumerate() {
                let found_author = author_name == score.username.to_lowercase();
                let mut username = String::with_capacity(32);
                if found_author {
                    username.push_str("__");
                }
                let _ = write!(
                    username,
                    "[{name}]({base}users/{id})",
                    name = score.username,
                    base = HOMEPAGE,
                    id = score.user_id
                );
                if found_author {
                    username.push_str("__");
                }
                let _ = writeln!(
                    description,
                    "**{idx}.** {grade} **{name}**: {score} [ {combo} ]{mods}\n\
                    - {pp} ~ {acc}% ~ {ago}",
                    idx = idx + i + 1,
                    grade = osu::grade_emote(score.grade, cache_data.cache())
                        .await
                        .to_string(),
                    name = username,
                    score = with_comma_u64(score.score as u64),
                    combo = get_combo(&score, &map),
                    mods = if score.enabled_mods.is_empty() {
                        String::new()
                    } else {
                        format!(" **+{}**", score.enabled_mods)
                    },
                    pp = get_pp(&mut mod_map, &score, &map, cache_data.data()).await?,
                    acc = round(score.accuracy),
                    ago = how_long_ago(&score.date),
                );
            }
            description
        } else {
            "No scores found".to_string()
        };
        let mut author = Author::new(author_text).url(format!("{}b/{}", HOMEPAGE, map.beatmap_id));
        if let Some(ref author_icon) = author_icon {
            author = author.icon_url(author_icon.to_owned());
        }
        Ok(Self {
            author,
            description,
            footer: Footer::new(footer_text).icon_url(footer_url),
            thumbnail: format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id),
        })
    }
}

impl EmbedData for LeaderboardEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn thumbnail(&self) -> Option<&str> {
        Some(&self.thumbnail)
    }
}

async fn get_pp(
    mod_map: &mut HashMap<u32, f32>,
    score: &ScraperScore,
    map: &Beatmap,
    data: &RwLock<TypeMap>,
) -> Result<String, Error> {
    let bits = score.enabled_mods.bits();
    let actual = if score.pp.is_some() {
        score.pp
    } else {
        match map.mode {
            GameMode::STD | GameMode::TKO => {
                Some(PPProvider::calculate_oppai_pp(score, map).await?)
            }
            GameMode::MNA | GameMode::CTB => {
                Some(PPProvider::calculate_pp(score, map, &data).await?)
            }
        }
    };
    #[allow(clippy::map_entry)]
    let max = if mod_map.contains_key(&bits) {
        mod_map.get(&bits).copied()
    } else {
        let max = PPProvider::calculate_max(&map, score.enabled_mods, Some(data)).await?;
        mod_map.insert(bits, max);
        Some(max)
    };
    Ok(format!(
        "**{}**/{}PP",
        actual.map_or_else(|| "-".to_string(), |pp| round(pp).to_string()),
        max.map_or_else(|| "-".to_string(), |pp| round(pp).to_string())
    ))
}

fn get_combo(score: &ScraperScore, map: &Beatmap) -> String {
    let mut combo = format!("**{}x**/", score.max_combo.to_string());
    let _ = if let Some(amount) = map.max_combo {
        write!(combo, "{}x", amount)
    } else {
        write!(
            combo,
            " {} miss{}",
            score.count_miss,
            if score.count_miss != 1 { "es" } else { "" }
        )
    };
    combo
}
