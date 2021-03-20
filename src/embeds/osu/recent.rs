use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        constants::{AVATAR_URL, DARK_GREEN, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago,
        error::PPError,
        matcher::highlight_funny_numeral,
        numbers::{round, with_comma_u64},
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
use twilight_embed_builder::{
    author::EmbedAuthorBuilder, builder::EmbedBuilder, image_source::ImageSource,
};
use twilight_model::channel::embed::EmbedField;

#[derive(Clone)]
pub struct RecentEmbed {
    description: Option<Cow<'static, str>>,
    title: String,
    url: String,
    author: Author,
    footer: Footer,
    timestamp: DateTime<Utc>,
    thumbnail: ImageSource,
    image: ImageSource,

    stars: f32,
    grade_completion_mods: String,
    score: String,
    acc: f32,
    ago: String,
    pp: String,
    combo: String,
    hits: String,
    if_fc: Option<(String, f32, String)>,
    map_info: String,
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

        let max_pp = max_result.pp;
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
                osu::get_keys(score.mods, &map),
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
                let _ = write!(description, "Global Top #{}", idx + 1);
            }

            description.push_str("**__");

            Some(description.into())
        } else {
            extend_desc
                .then(|| score_cmp_description(score, map_score))
                .flatten()
        };

        let image = ImageSource::url(format!(
            "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
            map.mapset_id
        ));

        Ok(Self {
            description,
            title,
            url: format!("{}b/{}", OSU_BASE, map.map_id),
            author: author!(user),
            footer,
            timestamp: score.created_at,
            thumbnail: ImageSource::url(format!("{}{}l.jpg", MAP_THUMB_URL, map.mapset_id))
                .unwrap(),
            image: image.unwrap(),
            grade_completion_mods,
            stars,
            score: with_comma_u64(score.score as u64),
            acc: round(score.accuracy),
            ago: how_long_ago(&score.created_at),
            pp,
            combo,
            hits,
            map_info: osu::get_map_info(&map),
            if_fc,
        })
    }
}

impl EmbedData for RecentEmbed {
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }

    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }

    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }

    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }

    fn image(&self) -> Option<&ImageSource> {
        Some(&self.image)
    }

    fn timestamp(&self) -> Option<&DateTime<Utc>> {
        Some(&self.timestamp)
    }

    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        let score = highlight_funny_numeral(&self.score).into_owned();
        let acc = highlight_funny_numeral(&format!("{}%", self.acc)).into_owned();
        let pp = highlight_funny_numeral(&self.pp).into_owned();

        let mut fields = vec![
            ("Grade".to_owned(), self.grade_completion_mods.clone(), true),
            ("Score".to_owned(), score, true),
            ("Acc".to_owned(), acc, true),
            ("PP".to_owned(), pp, true),
        ];

        fields.reserve(3 + (self.if_fc.is_some() as usize) * 3);

        let mania = self.hits.chars().filter(|&c| c == '/').count() == 5;

        let combo = highlight_funny_numeral(&self.combo).into_owned();
        let hits = highlight_funny_numeral(&self.hits).into_owned();

        fields.push((
            if mania { "Combo / Ratio" } else { "Combo" }.to_owned(),
            combo,
            true,
        ));

        fields.push(("Hits".to_owned(), hits, true));

        if let Some((pp, acc, hits)) = &self.if_fc {
            fields.push(("**If FC**: PP".to_owned(), pp.clone(), true));
            fields.push(("Acc".to_owned(), format!("{}%", acc), true));
            fields.push(("Hits".to_owned(), hits.clone(), true));
        }

        fields.push(("Map Info".to_owned(), self.map_info.clone(), false));

        Some(fields)
    }

    fn minimize(self) -> EmbedBuilder {
        let mut eb = EmbedBuilder::new();

        let name = format!(
            "{}\t{}\t({}%)\t{}",
            self.grade_completion_mods, self.score, self.acc, self.ago
        );

        let value = format!("{} [ {} ] {}", self.pp, self.combo, self.hits);
        let title = format!("{} [{}★]", self.title, self.stars);

        if let Some(description) = self.description {
            eb = eb.description(description).unwrap();
        }

        let ab = EmbedAuthorBuilder::new()
            .name(self.author.name)
            .unwrap()
            .url(self.author.url.unwrap())
            .icon_url(self.author.icon_url.unwrap());

        eb.color(DARK_GREEN)
            .unwrap()
            .thumbnail(self.thumbnail)
            .title(title)
            .unwrap()
            .url(self.url)
            .field(EmbedField {
                name,
                value,
                inline: false,
            })
            .author(ab)
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

            let pp_result = OsuPP::new(&map)
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

            let pp_result = FruitsPP::new(&map)
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

            let pp_result = TaikoPP::new(&map)
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

fn score_cmp_description(
    score: &Score,
    map_score: Option<&BeatmapUserScore>,
) -> Option<Cow<'static, str>> {
    if let Some(s) = map_score.map(|s| &s.score) {
        if s == score {
            return Some(Cow::Borrowed("Personal best on the map"));
        } else if score.score > s.score {
            let msg = if s.grade == Grade::F {
                "Would have been a personal best on the map"
            } else {
                "Personal best on the map"
            };

            return Some(Cow::Borrowed(msg));
        } else if s.grade != Grade::F {
            let msg = format!(
                "Missing {} score for a personal best on the map",
                with_comma_u64((s.score - score.score + 1) as u64)
            );

            return Some(Cow::Owned(msg));
        }
    }

    None
}
