use crate::{
    custom_client::ScraperScore,
    embeds::{Author, Footer},
    error::PPError,
    util::{
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago_dynamic,
        numbers::with_comma_int,
        osu::prepare_beatmap_file,
        CowUtils, ScoreExt,
    },
    BotResult,
};

use hashbrown::HashMap;
use rosu_pp::{
    Beatmap as Map, BeatmapExt, DifficultyAttributes, FruitsPP, GameMode as Mode, ManiaPP, OsuPP,
    PerformanceAttributes, TaikoPP,
};
use rosu_v2::prelude::{Beatmap, BeatmapsetCompact, GameMode};
use std::fmt::{self, Write};

pub struct LeaderboardEmbed {
    description: String,
    thumbnail: String,
    author: Author,
    footer: Footer,
}

impl LeaderboardEmbed {
    pub async fn new<'i, S>(
        author_name: Option<&str>,
        map: &Beatmap,
        mapset: Option<&BeatmapsetCompact>,
        scores: Option<S>,
        author_icon: &Option<String>,
        idx: usize,
    ) -> BotResult<Self>
    where
        S: Iterator<Item = &'i ScraperScore>,
    {
        let (artist, title, creator_name, creator_id) = match map.mapset {
            Some(ref ms) => (&ms.artist, &ms.title, &ms.creator_name, ms.creator_id),
            None => {
                let ms = mapset.expect("mapset neither in map nor in option");

                (&ms.artist, &ms.title, &ms.creator_name, ms.creator_id)
            }
        };

        let mut author_text = String::with_capacity(32);

        if map.mode == GameMode::MNA {
            let _ = write!(author_text, "[{}K] ", map.cs as u32);
        }

        let _ = write!(
            author_text,
            "{} - {} [{}] [{:.2}★]",
            artist, title, map.version, map.stars
        );

        let description = if let Some(scores) = scores {
            let map_path = prepare_beatmap_file(map.map_id).await?;
            let rosu_map = Map::from_path(map_path).await.map_err(PPError::from)?;

            let mut mod_map = HashMap::new();
            let mut description = String::with_capacity(256);
            let author_name = author_name.unwrap_or_default();

            for (i, score) in scores.enumerate() {
                let found_author = author_name == score.username;
                let mut username = String::with_capacity(32);

                if found_author {
                    username.push_str("__");
                }

                let _ = write!(
                    username,
                    "[{name}]({base}users/{id})",
                    name = score.username.cow_replace('_', "\\_"),
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
                    score = with_comma_int(score.score),
                    combo = get_combo(score, map),
                    mods = if score.mods.is_empty() {
                        String::new()
                    } else {
                        format!(" **+{}**", score.mods)
                    },
                    pp = get_pp(&mut mod_map, score, &rosu_map).await,
                    acc = score.accuracy,
                    ago = how_long_ago_dynamic(&score.date),
                );
            }

            description
        } else {
            "No scores found".to_string()
        };

        let mut author = Author::new(author_text).url(format!("{}b/{}", OSU_BASE, map.map_id));

        if let Some(ref author_icon) = author_icon {
            author = author.icon_url(author_icon.to_owned());
        }

        let footer = Footer::new(format!("{:?} map by {}", map.status, creator_name))
            .icon_url(format!("{}{}", AVATAR_URL, creator_id));

        Ok(Self {
            author,
            description,
            footer,
            thumbnail: format!("{}{}l.jpg", MAP_THUMB_URL, map.mapset_id),
        })
    }
}

impl_builder!(LeaderboardEmbed {
    author,
    description,
    footer,
    thumbnail,
});

async fn get_pp(
    mod_map: &mut HashMap<u32, (DifficultyAttributes, f32)>,
    score: &ScraperScore,
    map: &Map,
) -> PPFormatter {
    let bits = score.mods.bits();

    let (mut attributes, mut max_pp) = mod_map.remove(&bits).map_or_else(
        || {
            let attributes = map.stars(bits, None);

            (attributes, None)
        },
        |(attributes, max_pp)| (attributes, Some(max_pp)),
    );

    if max_pp.is_none() {
        let result: PerformanceAttributes = match map.mode {
            Mode::STD => OsuPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate()
                .into(),
            Mode::MNA => ManiaPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate()
                .into(),
            Mode::CTB => FruitsPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate()
                .into(),
            Mode::TKO => TaikoPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate()
                .into(),
        };

        max_pp.replace(result.pp() as f32);
        attributes = result.into();
    }

    let result: PerformanceAttributes = match map.mode {
        Mode::STD => OsuPP::new(map)
            .mods(bits)
            .attributes(attributes)
            .misses(score.count_miss as usize)
            .n300(score.count300 as usize)
            .n100(score.count100 as usize)
            .n50(score.count50 as usize)
            .combo(score.max_combo as usize)
            .calculate()
            .into(),
        Mode::MNA => ManiaPP::new(map)
            .mods(bits)
            .attributes(attributes)
            .score(score.score)
            .calculate()
            .into(),
        Mode::CTB => FruitsPP::new(map)
            .mods(bits)
            .attributes(attributes)
            .misses(score.count_miss as usize)
            .combo(score.max_combo as usize)
            .accuracy(score.accuracy as f64)
            .calculate()
            .into(),
        Mode::TKO => TaikoPP::new(map)
            .mods(bits)
            .attributes(attributes)
            .misses(score.count_miss as usize)
            .combo(score.max_combo as usize)
            .accuracy(score.accuracy as f64)
            .calculate()
            .into(),
    };

    let max_pp = max_pp.unwrap();
    let pp = result.pp() as f32;
    let attributes = result.into();

    mod_map.insert(bits, (attributes, max_pp));

    PPFormatter(pp, max_pp)
}

struct PPFormatter(f32, f32);

impl fmt::Display for PPFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "**{:.2}**/{:.2}PP", self.0, self.1)
    }
}

fn get_combo<'a>(score: &'a ScraperScore, map: &'a Beatmap) -> ComboFormatter<'a> {
    ComboFormatter(score, map)
}

struct ComboFormatter<'a>(&'a ScraperScore, &'a Beatmap);

impl<'a> fmt::Display for ComboFormatter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "**{}x**/", self.0.max_combo)?;

        if let Some(amount) = self.1.max_combo {
            write!(f, "{}x", amount)?;
        } else {
            write!(f, " {} miss", self.0.count_miss,)?;

            if self.0.count_miss != 1 {
                f.write_str("es")?;
            }
        }

        Ok(())
    }
}
