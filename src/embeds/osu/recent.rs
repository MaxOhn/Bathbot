use crate::{
    embeds::{osu, Author, EmbedBuilder, EmbedData, Footer},
    util::{
        constants::AVATAR_URL,
        datetime::{how_long_ago_dynamic, HowLongAgoFormatterDynamic},
        error::PPError,
        matcher::highlight_funny_numeral,
        numbers::{round, with_comma_uint},
        osu::{grade_completion_mods, prepare_beatmap_file},
        ScoreExt,
    },
    BotResult,
};

use chrono::{DateTime, Utc};
use rosu_pp::{Beatmap as Map, BeatmapExt, FruitsPP, ManiaPP, OsuPP, StarResult, TaikoPP};
use rosu_v2::prelude::{BeatmapUserScore, GameMode, Grade, Score, User};
use std::{borrow::Cow, fmt::Write};
use tokio::fs::File;

#[derive(Clone)]
pub struct RecentEmbed {
    description: String,
    title: String,
    url: String,
    author: Author,
    footer: Footer,
    timestamp: DateTime<Utc>,
    thumbnail: String,

    stars: f32,
    grade_completion_mods: Cow<'static, str>,
    score: String,
    acc: f32,
    ago: HowLongAgoFormatterDynamic,
    pp: String,
    combo: String,
    hits: String,
    if_fc: Option<(String, f32, String)>,
    map_info: String,
    mapset_cover: String,
}

impl RecentEmbed {
    pub async fn new(
        user: &User,
        score: &Score,
        personal: Option<&[Score]>,
        map_score: Option<&BeatmapUserScore>,
        extend_desc: bool,
    ) -> BotResult<Self> {
        let map = score.map.as_ref().unwrap();
        let mapset = score.mapset.as_ref().unwrap();

        let map_path = prepare_beatmap_file(map.map_id).await?;
        let file = File::open(map_path).await.map_err(PPError::from)?;
        let rosu_map = Map::parse(file).await.map_err(PPError::from)?;
        let mods = score.mods.bits();
        let max_result = rosu_map.max_pp(mods);
        let mut attributes = max_result.attributes;

        let max_pp = score
            .pp
            .filter(|pp| {
                score.grade.eq_letter(Grade::X) && score.mode != GameMode::MNA && *pp > 0.0
            })
            .unwrap_or(max_result.pp);

        let stars = round(attributes.stars());

        let pp = if let Some(pp) = score.pp {
            pp
        } else if score.grade == Grade::F {
            let hits = score.total_hits() as usize;

            let pp_result = match map.mode {
                GameMode::STD => OsuPP::new(&rosu_map)
                    .mods(mods)
                    .combo(score.max_combo as usize)
                    .n300(score.statistics.count_300 as usize)
                    .n100(score.statistics.count_100 as usize)
                    .n50(score.statistics.count_50 as usize)
                    .misses(score.statistics.count_miss as usize)
                    .passed_objects(hits)
                    .calculate(),
                GameMode::MNA => ManiaPP::new(&rosu_map)
                    .mods(mods)
                    .score(score.score)
                    .passed_objects(hits)
                    .calculate(),
                GameMode::CTB => FruitsPP::new(&rosu_map)
                    .mods(mods)
                    .combo(score.max_combo as usize)
                    .fruits(score.statistics.count_300 as usize)
                    .droplets(score.statistics.count_100 as usize)
                    .misses(score.statistics.count_miss as usize)
                    .passed_objects(hits - score.statistics.count_katu as usize)
                    .accuracy(score.accuracy)
                    .calculate(),
                GameMode::TKO => TaikoPP::new(&rosu_map)
                    .combo(score.max_combo as usize)
                    .mods(mods)
                    .passed_objects(hits)
                    .accuracy(score.accuracy)
                    .calculate(),
            };

            pp_result.pp
        } else {
            let pp_result = match map.mode {
                GameMode::STD => OsuPP::new(&rosu_map)
                    .attributes(attributes)
                    .mods(mods)
                    .combo(score.max_combo as usize)
                    .n300(score.statistics.count_300 as usize)
                    .n100(score.statistics.count_100 as usize)
                    .n50(score.statistics.count_50 as usize)
                    .misses(score.statistics.count_miss as usize)
                    .calculate(),
                GameMode::MNA => ManiaPP::new(&rosu_map)
                    .attributes(attributes)
                    .mods(mods)
                    .score(score.score)
                    .calculate(),
                GameMode::CTB => FruitsPP::new(&rosu_map)
                    .attributes(attributes)
                    .mods(mods)
                    .combo(score.max_combo as usize)
                    .fruits(score.statistics.count_300 as usize)
                    .droplets(score.statistics.count_100 as usize)
                    .misses(score.statistics.count_miss as usize)
                    .accuracy(score.accuracy)
                    .calculate(),
                GameMode::TKO => TaikoPP::new(&rosu_map)
                    .attributes(attributes)
                    .combo(score.max_combo as usize)
                    .mods(mods)
                    .misses(score.statistics.count_miss as usize)
                    .accuracy(score.accuracy)
                    .calculate(),
            };

            attributes = pp_result.attributes;

            pp_result.pp
        };

        let if_fc = if_fc_struct(score, &rosu_map, attributes, mods);

        let pp = osu::get_pp(Some(pp), Some(max_pp));
        let hits = score.hits_string(map.mode);
        let grade_completion_mods = grade_completion_mods(score, map);

        let (combo, title) = if map.mode == GameMode::MNA {
            let mut ratio = score.statistics.count_geki as f32;

            if score.statistics.count_300 > 0 {
                ratio /= score.statistics.count_300 as f32
            }

            let combo = format!("**{}x** / {:.2}", &score.max_combo, ratio);

            let title = format!(
                "{} {} - {} [{}]",
                osu::get_keys(score.mods, map),
                mapset.artist,
                mapset.title,
                map.version
            );

            (combo, title)
        } else {
            (
                osu::get_combo(score, map),
                format!("{} - {} [{}]", mapset.artist, mapset.title, map.version),
            )
        };

        let if_fc = if_fc.map(|if_fc| {
            let mut hits = String::from("{");
            let _ = write!(hits, "{}/", if_fc.n300);
            let _ = write!(hits, "{}/", if_fc.n100);

            if let Some(n50) = if_fc.n50 {
                let _ = write!(hits, "{}/", n50);
            }

            let _ = write!(hits, "0}}");

            (
                osu::get_pp(Some(if_fc.pp), Some(max_pp)),
                round(if_fc.acc),
                hits,
            )
        });

        let footer = Footer::new(format!(
            "{:?} map by {} | played",
            map.status, mapset.creator_name
        ))
        .icon_url(format!("{}{}", AVATAR_URL, mapset.creator_id));

        let personal_idx = personal.and_then(|personal| personal.iter().position(|s| s == score));

        let global_idx = map_score
            .and_then(|s| (&s.score == score).then(|| s.pos))
            .filter(|&p| p <= 50);

        let description = if personal_idx.is_some() || global_idx.is_some() {
            let mut description = String::with_capacity(25);
            description.push_str("__**");

            if let Some(idx) = personal_idx {
                let _ = write!(description, "Personal Best #{}", idx + 1);

                if global_idx.is_some() {
                    description.reserve(19);
                    description.push_str(" and ");
                }
            }

            if let Some(idx) = global_idx {
                let _ = write!(description, "Global Top #{}", idx);
            }

            description.push_str("**__");

            description
        } else {
            extend_desc
                .then(|| score_cmp_description(score, map_score))
                .flatten()
                .unwrap_or_default()
        };

        Ok(Self {
            description,
            title,
            url: map.url.to_owned(),
            author: author!(user),
            footer,
            timestamp: score.created_at,
            thumbnail: mapset.covers.list.to_owned(),
            grade_completion_mods,
            stars,
            score: with_comma_uint(score.score).to_string(),
            acc: round(score.accuracy),
            ago: how_long_ago_dynamic(&score.created_at),
            pp,
            combo,
            hits,
            map_info: osu::get_map_info(map, score.mods, stars),
            if_fc,
            mapset_cover: mapset.covers.cover.to_owned(),
        })
    }
}

impl EmbedData for RecentEmbed {
    fn as_builder(&self) -> EmbedBuilder {
        let score = highlight_funny_numeral(&self.score).into_owned();
        let acc = highlight_funny_numeral(&format!("{}%", self.acc)).into_owned();
        let pp = highlight_funny_numeral(&self.pp).into_owned();

        let mut fields = vec![
            field!(
                "Grade",
                self.grade_completion_mods.as_ref().to_owned(),
                true
            ),
            field!("Score", score, true),
            field!("Acc", acc, true),
            field!("PP", pp, true),
        ];

        fields.reserve(3 + (self.if_fc.is_some() as usize) * 3);

        let mania = self.hits.chars().filter(|&c| c == '/').count() == 5;

        let combo = highlight_funny_numeral(&self.combo).into_owned();
        let hits = highlight_funny_numeral(&self.hits).into_owned();

        let name = if mania { "Combo / Ratio" } else { "Combo" };

        fields.push(field!(name, combo, true));
        fields.push(field!("Hits", hits, true));

        if let Some((pp, acc, hits)) = &self.if_fc {
            fields.push(field!("**If FC**: PP", pp.clone(), true));
            fields.push(field!("Acc", format!("{}%", acc), true));
            fields.push(field!("Hits", hits.clone(), true));
        }

        fields.push(field!("Map Info".to_owned(), self.map_info.clone(), false));

        EmbedBuilder::new()
            .author(&self.author)
            .description(&self.description)
            .fields(fields)
            .footer(&self.footer)
            .image(&self.mapset_cover)
            .timestamp(self.timestamp)
            .title(&self.title)
            .url(&self.url)
    }

    fn into_builder(self) -> EmbedBuilder {
        let name = format!(
            "{}\t{}\t({}%)\t{}",
            self.grade_completion_mods, self.score, self.acc, self.ago
        );

        let value = format!("{} [ {} ] {}", self.pp, self.combo, self.hits);

        let mut title = self.title;
        let _ = write!(title, " [{}★]", self.stars);

        EmbedBuilder::new()
            .author(self.author)
            .description(self.description)
            .fields(vec![field!(name, value, false)])
            .thumbnail(self.thumbnail)
            .title(title)
            .url(self.url)
    }
}

struct IfFC {
    n300: usize,
    n100: usize,
    n50: Option<usize>,
    pp: f32,
    acc: f32,
}

fn if_fc_struct(score: &Score, map: &Map, attributes: StarResult, mods: u32) -> Option<IfFC> {
    match attributes {
        StarResult::Osu(attributes)
            if score.statistics.count_miss > 0
                || score.max_combo < attributes.max_combo as u32 - 5 =>
        {
            let total_objects = (map.n_circles + map.n_sliders + map.n_spinners) as usize;
            let passed_objects = (score.statistics.count_300
                + score.statistics.count_100
                + score.statistics.count_50
                + score.statistics.count_miss) as usize;

            let mut count300 =
                score.statistics.count_300 as usize + total_objects.saturating_sub(passed_objects);

            let count_hits = total_objects - score.statistics.count_miss as usize;
            let ratio = 1.0 - (count300 as f32 / count_hits as f32);
            let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

            count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
            let count100 = (score.statistics.count_100 + new100s) as usize;
            let count50 = score.statistics.count_50 as usize;

            let pp_result = OsuPP::new(map)
                .attributes(attributes)
                .mods(mods)
                .n300(count300)
                .n100(count100)
                .n50(count50)
                .calculate();

            let acc =
                100.0 * (6 * count300 + 2 * count100 + count50) as f32 / (6 * total_objects) as f32;

            Some(IfFC {
                n300: count300,
                n100: count100,
                n50: Some(count50),
                pp: pp_result.pp,
                acc,
            })
        }
        StarResult::Fruits(attributes) if score.max_combo != attributes.max_combo as u32 => {
            let total_objects = attributes.max_combo;
            let passed_objects = (score.statistics.count_300
                + score.statistics.count_100
                + score.statistics.count_miss) as usize;

            let missing = total_objects - passed_objects;
            let missing_fruits = missing.saturating_sub(
                attributes
                    .n_droplets
                    .saturating_sub(score.statistics.count_100 as usize),
            );

            let missing_droplets = missing - missing_fruits;

            let n_fruits = score.statistics.count_300 as usize + missing_fruits;
            let n_droplets = score.statistics.count_100 as usize + missing_droplets;
            let n_tiny_droplet_misses = score.statistics.count_katu as usize;
            let n_tiny_droplets = attributes
                .n_tiny_droplets
                .saturating_sub(n_tiny_droplet_misses);

            let pp_result = FruitsPP::new(map)
                .attributes(attributes)
                .mods(mods)
                .fruits(n_fruits)
                .droplets(n_droplets)
                .tiny_droplets(n_tiny_droplets)
                .tiny_droplet_misses(n_tiny_droplet_misses)
                .calculate();

            let hits = n_fruits + n_droplets + n_tiny_droplets;
            let total = hits + n_tiny_droplet_misses;

            let acc = if total == 0 {
                0.0
            } else {
                100.0 * hits as f32 / total as f32
            };

            Some(IfFC {
                n300: n_fruits,
                n100: n_droplets,
                n50: Some(n_tiny_droplets),
                pp: pp_result.pp,
                acc,
            })
        }
        StarResult::Taiko(attributes)
            if score.grade == Grade::F || score.statistics.count_miss > 0 =>
        {
            let total_objects = map.n_circles as usize;
            let passed_objects = score.total_hits() as usize;

            let mut count300 =
                score.statistics.count_300 as usize + total_objects.saturating_sub(passed_objects);

            let count_hits = total_objects - score.statistics.count_miss as usize;
            let ratio = 1.0 - (count300 as f32 / count_hits as f32);
            let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

            count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
            let count100 = (score.statistics.count_100 + new100s) as usize;

            let acc = 100.0 * (2 * count300 + count100) as f32 / (2 * total_objects) as f32;

            let pp_result = TaikoPP::new(map)
                .attributes(attributes)
                .mods(mods)
                .accuracy(acc)
                .calculate();

            Some(IfFC {
                n300: count300,
                n100: count100,
                n50: None,
                pp: pp_result.pp,
                acc,
            })
        }
        _ => None,
    }
}

fn score_cmp_description(score: &Score, map_score: Option<&BeatmapUserScore>) -> Option<String> {
    let s = map_score.map(|s| &s.score)?;

    if s == score {
        Some("Personal best on the map".to_owned())
    } else if score.score > s.score {
        let msg = if s.grade == Grade::F {
            "Would have been a personal best on the map"
        } else {
            "Personal best on the map"
        };

        Some(msg.to_owned())
    } else if s.grade != Grade::F {
        let msg = format!(
            "Missing {} score for a personal best on the map",
            with_comma_uint(s.score - score.score + 1)
        );

        Some(msg)
    } else {
        None
    }
}
