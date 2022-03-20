use crate::{
    core::Context,
    database::MinimizedPp,
    embeds::{osu, Author, EmbedBuilder, EmbedData, Footer},
    error::PpError,
    util::{
        constants::AVATAR_URL,
        datetime::{how_long_ago_dynamic, HowLongAgoFormatterDynamic},
        numbers::{round, with_comma_int},
        osu::{grade_completion_mods, prepare_beatmap_file},
        ScoreExt,
    },
    BotResult,
};

use chrono::{DateTime, Utc};
use rosu_pp::{Beatmap as Map, BeatmapExt, CatchPP, DifficultyAttributes, OsuPP, TaikoPP};
use rosu_v2::prelude::{GameMode, Grade, Score, User};
use std::{borrow::Cow, fmt::Write};

#[derive(Clone)]
pub struct TopSingleEmbed {
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
    pp: Option<f32>,
    max_pp: Option<f32>,
    combo: String,
    hits: String,
    if_fc: Option<(f32, f32, String)>,
    map_info: String,
    mapset_cover: String,
    minimized_pp: MinimizedPp,
}

impl TopSingleEmbed {
    pub async fn new(
        user: &User,
        score: &Score,
        personal_idx: Option<usize>,
        global_idx: Option<usize>,
        minimized_pp: MinimizedPp,
        ctx: &Context,
    ) -> BotResult<Self> {
        let map = score.map.as_ref().unwrap();
        let mapset = score.mapset.as_ref().unwrap();

        let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
        let rosu_map = Map::from_path(map_path).await.map_err(PpError::from)?;
        let mods = score.mods.bits();
        let max_result = rosu_map.max_pp(mods);
        let attributes = max_result.difficulty_attributes();

        let max_pp = score
            .pp
            .filter(|pp| {
                score.grade.eq_letter(Grade::X) && score.mode != GameMode::MNA && *pp > 0.0
            })
            .unwrap_or(max_result.pp() as f32);

        let stars = round(attributes.stars() as f32);

        let if_fc = if_fc_struct(score, &rosu_map, attributes, mods);

        let pp = score.pp;
        let hits = score.hits_string(score.mode);
        let grade_completion_mods = grade_completion_mods(score, map);

        let (combo, title) = if score.mode == GameMode::MNA {
            let mut ratio = score.statistics.count_geki as f32;

            if score.statistics.count_300 > 0 {
                ratio /= score.statistics.count_300 as f32
            }

            let combo = format!("**{}x** / {ratio:.2}", &score.max_combo);

            let title = format!(
                "{} {} - {} [{}]",
                osu::get_keys(score.mods, map),
                mapset.artist,
                mapset.title,
                map.version
            );

            (combo, title)
        } else {
            let title = format!("{} - {} [{}]", mapset.artist, mapset.title, map.version);

            (osu::get_combo(score, map), title)
        };

        let if_fc = if_fc.map(|if_fc| {
            let mut hits = String::from("{");
            let _ = write!(hits, "{}/", if_fc.n300);
            let _ = write!(hits, "{}/", if_fc.n100);

            if let Some(n50) = if_fc.n50 {
                let _ = write!(hits, "{n50}/");
            }

            let _ = write!(hits, "0}}");

            (if_fc.pp, round(if_fc.acc), hits)
        });

        let footer = Footer::new(format!(
            "{:?} map by {} | played",
            map.status, mapset.creator_name
        ))
        .icon_url(format!("{AVATAR_URL}{}", mapset.creator_id));

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

            description
        } else {
            String::new()
        };

        Ok(Self {
            title,
            footer,
            thumbnail: mapset.covers.list.to_owned(),
            description,
            url: map.url.to_owned(),
            author: author!(user),
            timestamp: score.created_at,
            grade_completion_mods,
            stars,
            score: with_comma_int(score.score).to_string(),
            acc: round(score.accuracy),
            ago: how_long_ago_dynamic(&score.created_at),
            pp,
            max_pp: Some(max_pp),
            combo,
            hits,
            map_info: osu::get_map_info(map, score.mods, stars),
            if_fc,
            mapset_cover: mapset.covers.cover.to_owned(),
            minimized_pp,
        })
    }
}

impl EmbedData for TopSingleEmbed {
    fn as_builder(&self) -> EmbedBuilder {
        let pp = osu::get_pp(self.pp, self.max_pp);

        let mut fields = vec![
            field!(
                "Grade",
                self.grade_completion_mods.as_ref().to_owned(),
                true
            ),
            field!("Score", self.score.clone(), true),
            field!("Acc", format!("{}%", self.acc), true),
            field!("PP", pp, true),
        ];

        let mania = self.hits.chars().filter(|&c| c == '/').count() == 5;

        fields.push(field!(
            if mania { "Combo / Ratio" } else { "Combo" },
            self.combo.clone(),
            true
        ));

        fields.push(field!("Hits", self.hits.clone(), true));

        if let Some((pp, acc, hits)) = &self.if_fc {
            let pp = osu::get_pp(Some(*pp), self.max_pp);
            fields.push(field!("**If FC**: PP", pp, true));
            fields.push(field!("Acc", format!("{acc}%"), true));
            fields.push(field!("Hits", hits.clone(), true));
        }

        fields.push(field!("Map Info", self.map_info.clone(), false));

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

        let pp = match self.minimized_pp {
            MinimizedPp::IfFc => {
                let mut result = String::with_capacity(17);
                result.push_str("**");

                if let Some(pp) = self.pp {
                    let _ = write!(result, "{:.2}", pp);
                } else {
                    result.push('-');
                }

                match self.if_fc {
                    Some((if_fc, ..)) => {
                        let _ = write!(result, "pp** ~~({if_fc:.2}pp)~~");
                    }
                    None => {
                        result.push_str("**/");

                        if let Some(max) = self.max_pp {
                            let pp = self.pp.map(|pp| pp.max(max)).unwrap_or(max);
                            let _ = write!(result, "{:.2}", pp);
                        } else {
                            result.push('-');
                        }

                        result.push_str("PP");
                    }
                }

                result
            }
            MinimizedPp::Max => osu::get_pp(self.pp, self.max_pp),
        };

        let value = format!("{pp} [ {} ] {}", self.combo, self.hits);

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

fn if_fc_struct(
    score: &Score,
    map: &Map,
    attributes: DifficultyAttributes,
    mods: u32,
) -> Option<IfFC> {
    match attributes {
        DifficultyAttributes::Osu(attributes)
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
                pp: pp_result.pp as f32,
                acc,
            })
        }
        DifficultyAttributes::Catch(attributes)
            if score.max_combo != attributes.max_combo() as u32 =>
        {
            let total_objects = attributes.max_combo();
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

            let pp_result = CatchPP::new(map)
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
                pp: pp_result.pp as f32,
                acc,
            })
        }
        DifficultyAttributes::Taiko(attributes) if score.statistics.count_miss > 0 => {
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
                .accuracy(acc as f64)
                .calculate();

            Some(IfFC {
                n300: count300,
                n100: count100,
                n50: None,
                pp: pp_result.pp as f32,
                acc,
            })
        }
        _ => None,
    }
}
