use crate::{util::globals::*, Error};
use roppai::Oppai;
use rosu::models::{GameMode, Grade, Score};
use serenity::{
    cache::CacheRwLock,
    model::{
        guild::Emoji,
        id::{EmojiId, GuildId},
    },
};
use std::{env, fs::File, io::Write, path::Path};
use tokio::runtime::Runtime;

const MAP_RETRIEVAL_URL: &str = "https://osu.ppy.sh/web/maps/";

pub fn get_oppai(map_id: u32, score: &Score, mode: GameMode) -> Result<(Oppai, f32), Error> {
    let mut oppai = Oppai::new();
    let bits = score.enabled_mods.get_bits();
    oppai.set_mode(mode as u8).set_mods(bits);
    let map_path = prepare_beatmap_file(map_id)?;

    // First calculate only the max pp of the map with the current mods
    // TODO: Check if value is already calculated in database
    let max_pp = oppai.calculate(Some(&map_path))?.get_pp();

    // Then set all values corresponding to the score so that the
    // caller can use the oppai isntance
    oppai
        .set_miss_count(score.count_miss)
        .set_hits(score.count100, score.count50)
        .set_end_index(score.get_amount_hits(mode))
        .set_combo(score.max_combo)
        .calculate(None)?;
    Ok((oppai, max_pp))
}

pub fn prepare_beatmap_file(map_id: u32) -> Result<String, Error> {
    let map_path = format!(
        "{base}{id}.osu",
        base = env::var("BEATMAP_PATH")?,
        id = map_id
    );
    if !Path::new(&map_path).exists() {
        let mut file = File::create(&map_path)?;
        let download_url = format!("{}{}", MAP_RETRIEVAL_URL, map_id);
        let mut rt = Runtime::new().unwrap();
        let content = rt.block_on(async { reqwest::get(&download_url).await?.text().await })?;
        file.write_all(content.as_bytes())?;
        debug!("Downloaded {}.osu successfully", map_id);
    }
    Ok(map_path)
}

pub fn get_grade_emote(grade: Grade, cache: CacheRwLock) -> Emoji {
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
