use crate::{
    custom_client::ScraperScore,
    embeds::{Author, EmbedData, Footer},
    util::{
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago,
        error::PPError,
        numbers::with_comma_u64,
        osu::prepare_beatmap_file,
        ScoreExt,
    },
    BotResult,
};

use cow_utils::CowUtils;
use rosu::model::{Beatmap, GameMode};
use rosu_pp::{
    Beatmap as Map, BeatmapExt, FruitsPP, GameMode as Mode, ManiaPP, OsuPP, StarResult, TaikoPP,
};
use std::{borrow::Cow, collections::HashMap, fmt::Write, fs::File};
use twilight_embed_builder::image_source::ImageSource;

pub struct LeaderboardEmbed {
    description: String,
    thumbnail: ImageSource,
    author: Author,
    footer: Footer,
}

impl LeaderboardEmbed {
    pub async fn new<'i, S>(
        init_name: Option<&str>,
        map: &Beatmap,
        scores: Option<S>,
        author_icon: &Option<String>,
        idx: usize,
    ) -> BotResult<Self>
    where
        S: Iterator<Item = &'i ScraperScore>,
    {
        let mut author_text = String::with_capacity(32);

        if map.mode == GameMode::MNA {
            let _ = write!(author_text, "[{}K] ", map.diff_cs as u32);
        }

        let _ = write!(author_text, "{} [{:.2}★]", map, map.stars);

        let description = if let Some(scores) = scores {
            let map_path = prepare_beatmap_file(map.beatmap_id).await?;
            let file = File::open(map_path).map_err(PPError::from)?;
            let rosu_map = Map::parse(file).map_err(PPError::from)?;

            let mut mod_map = HashMap::new();
            let mut description = String::with_capacity(256);
            let author_name = init_name.map_or_else(|| Cow::Borrowed(""), |n| n.cow_to_lowercase());

            for (i, score) in scores.enumerate() {
                let found_author = author_name == score.username.cow_to_lowercase();
                let mut username = String::with_capacity(32);

                if found_author {
                    username.push_str("__");
                }

                let _ = write!(
                    username,
                    "[{name}]({base}users/{id})",
                    name = score.username,
                    base = OSU_BASE,
                    id = score.user_id
                );

                if found_author {
                    username.push_str("__");
                }

                let _ = writeln!(
                    description,
                    "**{idx}.** {grade} **{name}**: {score} [ {combo} ]{mods}\n\
                    - {pp} ~ {acc:.2}% ~ {ago}",
                    idx = idx + i + 1,
                    grade = score.grade_emote(map.mode),
                    name = username,
                    score = with_comma_u64(score.score as u64),
                    combo = get_combo(&score, &map),
                    mods = if score.enabled_mods.is_empty() {
                        String::new()
                    } else {
                        format!(" **+{}**", score.enabled_mods)
                    },
                    pp = get_pp(&mut mod_map, &score, &rosu_map).await,
                    acc = score.accuracy,
                    ago = how_long_ago(&score.date),
                );
            }

            description
        } else {
            "No scores found".to_string()
        };

        let mut author = Author::new(author_text).url(format!("{}b/{}", OSU_BASE, map.beatmap_id));

        if let Some(ref author_icon) = author_icon {
            author = author.icon_url(author_icon.to_owned());
        }

        let footer = Footer::new(format!("{:?} map by {}", map.approval_status, map.creator))
            .icon_url(format!("{}{}", AVATAR_URL, map.creator_id));

        Ok(Self {
            author,
            description,
            footer,
            thumbnail: ImageSource::url(format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id))
                .unwrap(),
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

    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
}

async fn get_pp(
    mod_map: &mut HashMap<u32, (StarResult, f32)>,
    score: &ScraperScore,
    map: &Map,
) -> String {
    let bits = score.enabled_mods.bits();

    let (mut attributes, mut max_pp) = mod_map.remove(&bits).map_or_else(
        || {
            let attributes = map.stars(bits, None);

            (attributes, None)
        },
        |(attributes, max_pp)| (attributes, Some(max_pp)),
    );

    if max_pp.is_none() {
        let result = match map.mode {
            Mode::STD => OsuPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate(),
            Mode::MNA => ManiaPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate(),
            Mode::CTB => FruitsPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate(),
            Mode::TKO => TaikoPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate(),
        };

        max_pp.replace(result.pp());
        attributes = result.attributes;
    }

    let result = match map.mode {
        Mode::STD => OsuPP::new(map)
            .mods(bits)
            .attributes(attributes)
            .misses(score.count_miss as usize)
            .n300(score.count300 as usize)
            .n100(score.count100 as usize)
            .n50(score.count50 as usize)
            .combo(score.max_combo as usize)
            .calculate(),
        Mode::MNA => ManiaPP::new(map)
            .mods(bits)
            .attributes(attributes)
            .score(score.score)
            .calculate(),
        Mode::CTB => FruitsPP::new(map)
            .mods(bits)
            .attributes(attributes)
            .misses(score.count_miss as usize)
            .combo(score.max_combo as usize)
            .accuracy(score.accuracy)
            .calculate(),
        Mode::TKO => TaikoPP::new(map)
            .mods(bits)
            .attributes(attributes)
            .misses(score.count_miss as usize)
            .combo(score.max_combo as usize)
            .accuracy(score.accuracy)
            .calculate(),
    };

    let max_pp = max_pp.unwrap();
    let pp = result.pp();
    let attributes = result.attributes;

    mod_map.insert(bits, (attributes, max_pp));

    format!("**{:.2}**/{:.2}PP", pp, max_pp)
}

fn get_combo(score: &ScraperScore, map: &Beatmap) -> String {
    let mut combo = format!("**{}x**/", score.max_combo);
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
