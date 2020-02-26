use crate::{
    util::globals::{emotes::*, DEV_GUILD_ID, HOMEPAGE},
    Error,
};
use regex::Regex;
use rosu::models::{Beatmap, GameMod, GameMode, Grade, Score};
use serenity::{
    cache::CacheRwLock,
    model::{
        channel::{EmbedField, Message},
        guild::Emoji,
        id::{EmojiId, GuildId},
    },
};
use std::{env, fs::File, io::Write, path::Path, str::FromStr};
use tokio::runtime::Runtime;

pub fn map_id_from_history(msgs: Vec<Message>, cache: CacheRwLock) -> Option<u32> {
    let url_regex = Regex::new(r".*/([0-9]{1,9})").unwrap();
    let field_regex = Regex::new(r".*\{(\d+/){2,}\d+}.*").unwrap();
    let check_field = |url: &str, field: &EmbedField| {
        if field_regex.is_match(&field.value) {
            let caps = url_regex.captures(url).unwrap();
            if let Some(cap) = caps.get(1) {
                if let Ok(id) = u32::from_str(cap.as_str()) {
                    return Some(id);
                }
            }
        }
        None
    };
    for msg in msgs {
        if !msg.is_own(&cache) {
            continue;
        }
        for embed in msg.embeds {
            if let Some(author) = embed.author {
                if let Some(url) = author.url {
                    if url.contains("/b/") {
                        let caps = url_regex.captures(&url).unwrap();
                        if let Some(cap) = caps.get(1) {
                            if let Ok(id) = u32::from_str(cap.as_str()) {
                                return Some(id);
                            }
                        }
                    }
                }
            }
            if embed.fields.is_empty() {
                continue;
            }
            if let Some(url) = embed.url {
                if let Some(field) = embed.fields.first() {
                    if let Some(id) = check_field(&url, &field) {
                        return Some(id);
                    }
                }
                if let Some(field) = embed.fields.get(5) {
                    if let Some(id) = check_field(&url, &field) {
                        return Some(id);
                    }
                }
            }
        }
    }
    None
}

pub fn prepare_beatmap_file(map_id: u32) -> Result<String, Error> {
    let map_path = format!(
        "{base}{id}.osu",
        base = env::var("BEATMAP_PATH")?,
        id = map_id
    );
    if !Path::new(&map_path).exists() {
        let mut file = File::create(&map_path)?;
        let download_url = format!("{}web/maps/{}", HOMEPAGE, map_id);
        let mut rt = Runtime::new().unwrap();
        let content = rt.block_on(async { reqwest::get(&download_url).await?.text().await })?;
        file.write_all(content.as_bytes())?;
        debug!("Downloaded {}.osu successfully", map_id);
    }
    Ok(map_path)
}

pub fn grade_emote(grade: Grade, cache: CacheRwLock) -> Emoji {
    let emoji_id = match grade {
        Grade::XH => EmojiId(EMOTE_XH_ID),
        Grade::X => EmojiId(EMOTE_X_ID),
        Grade::SH => EmojiId(EMOTE_SH_ID),
        Grade::S => EmojiId(EMOTE_S_ID),
        Grade::A => EmojiId(EMOTE_A_ID),
        Grade::B => EmojiId(EMOTE_B_ID),
        Grade::C => EmojiId(EMOTE_C_ID),
        Grade::D => EmojiId(EMOTE_D_ID),
        Grade::F => EmojiId(EMOTE_F_ID),
    };
    let guild = GuildId(DEV_GUILD_ID).to_guild_cached(cache);
    guild
        .unwrap()
        .read()
        .emojis
        .get(&emoji_id)
        .unwrap_or_else(|| {
            panic!(
                "Could not find emote with id {} for grade {}",
                emoji_id.0, grade
            )
        })
        .clone()
}

/// Assumes the mode to be STD, otherwise might not work as intended
pub fn unchoke_score(score: &mut Score, map: &Beatmap) -> Result<(), Error> {
    match map.mode {
        GameMode::STD => {
            let max_combo = map
                .max_combo
                .unwrap_or_else(|| panic!("Max combo of beatmap not found"));
            if score.max_combo == max_combo {
                return Ok(());
            }
            let total_objects = map.count_objects();
            let passed_objects = score.total_hits(GameMode::STD);
            score.count300 += total_objects - passed_objects;
            let count_hits = total_objects - score.count_miss;
            let ratio = score.count300 as f32 / count_hits as f32;
            let new100s = (ratio * score.count_miss as f32) as u32;
            score.count100 += new100s;
            score.count300 += score.count_miss - new100s;
            score.max_combo = max_combo;
            score.count_miss = 0;
            score.recalculate_grade(GameMode::STD, None);
            Ok(())
        }
        GameMode::MNA => {
            score.max_combo = 0;
            score.score = 1_000_000;
            score.count_geki = map.count_objects();
            score.count300 = 0;
            score.count_katu = 0;
            score.count100 = 0;
            score.count50 = 0;
            score.count_miss = 0;
            score.grade = if score.enabled_mods.contains(&GameMod::Hidden) {
                Grade::XH
            } else {
                Grade::X
            };
            Ok(())
        }
        _ => Err(Error::Custom(format!(
            "Can only unchoke STD and MNA scores, not {:?}",
            map.mode,
        ))),
    }
}
