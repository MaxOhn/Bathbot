use crate::{
    arguments::SimulateArgs,
    embeds::{osu, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, DARK_GREEN, MAP_THUMB_URL, OSU_BASE},
        error::PPError,
        numbers::{round, with_comma_u64},
        osu::{grade_completion_mods, prepare_beatmap_file, ModSelection},
        ScoreExt,
    },
    BotResult,
};

use rosu::model::{Beatmap, GameMode, GameMods, Grade, Score};
use rosu_pp::{
    Beatmap as Map, BeatmapExt, FruitsPP, GameMode as Mode, ManiaPP, OsuPP, PpResult, StarResult,
    TaikoPP,
};
use std::{fmt::Write, fs::File};
use twilight_embed_builder::{builder::EmbedBuilder, image_source::ImageSource};
use twilight_model::channel::embed::EmbedField;

pub struct SimulateEmbed {
    title: String,
    url: String,
    footer: Footer,
    thumbnail: ImageSource,
    image: ImageSource,

    mode: GameMode,
    stars: f32,
    grade_completion_mods: String,
    acc: f32,
    prev_pp: Option<f32>,
    pp: String,
    prev_combo: Option<u32>,
    score: u64,
    combo: String,
    prev_hits: Option<String>,
    hits: String,
    removed_misses: Option<u32>,
    map_info: String,
}

impl SimulateEmbed {
    pub async fn new(score: Option<Score>, map: &Beatmap, args: SimulateArgs) -> BotResult<Self> {
        let is_some = args.is_some();

        let title = if map.mode == GameMode::MNA {
            format!("{} {}", osu::get_keys(GameMods::default(), map), map)
        } else {
            map.to_string()
        };

        let (prev_pp, prev_combo, prev_hits, misses) = if let Some(ref s) = score {
            let mut calculator = PPCalculator::new().score(s).map(map);
            calculator.calculate(Calculations::PP).await?;
            let prev_pp = Some(round(calculator.pp().unwrap_or(0.0)));

            let prev_combo = if map.mode == GameMode::STD {
                Some(s.max_combo)
            } else {
                None
            };

            let prev_hits = Some(s.hits_string(map.mode));

            (prev_pp, prev_combo, prev_hits, Some(s.count_miss))
        } else {
            (None, None, None, None)
        };

        let mut unchoked_score = score.unwrap_or_default();
        let map_path = prepare_beatmap_file(map.beatmap_id).await?;
        let file = File::open(map_path).map_err(PPError::from)?;
        let rosu_map = Map::parse(file).map_err(PPError::from)?;

        if let Some(ModSelection::Exact(mods)) | Some(ModSelection::Include(mods)) = args.mods {
            unchoked_score.enabled_mods = mods;
        }

        let PpResult {
            pp: max_pp,
            attributes,
        } = rosu_map.max_pp(unchoked_score.enabled_mods.bits());

        let stars = round(attributes.stars());

        let attributes = if is_some {
            simulate_score(&mut unchoked_score, map, args, attributes)
        } else {
            unchoke_score(&mut unchoked_score, &map, attributes)
        };

        let PpResult { pp, .. } = pp(&unchoked_score, &rosu_map, attributes);

        let grade_completion_mods = grade_completion_mods(&unchoked_score, map);
        let pp = osu::get_pp(Some(pp), Some(max_pp));
        let hits = unchoked_score.hits_string(map.mode);

        let (combo, acc) = match map.mode {
            GameMode::STD | GameMode::CTB => (
                osu::get_combo(&unchoked_score, map),
                round(unchoked_score.accuracy(map.mode)),
            ),
            GameMode::MNA => (String::from("**-**/-"), 100.0),
            GameMode::TKO => {
                let acc = round(unchoked_score.accuracy(GameMode::TKO));

                let combo = if is_some {
                    if unchoked_score.max_combo == 0 {
                        "**-**/-".to_owned()
                    } else {
                        format!("**{}x**/-", unchoked_score.max_combo)
                    }
                } else if let Some(combo) = map.max_combo {
                    format!("**{combo}x**/{combo}", combo = combo)
                } else {
                    "**-**/-".to_string()
                };

                (combo, acc)
            }
        };

        let footer = Footer::new(format!("{:?} map by {}", map.approval_status, map.creator))
            .icon_url(format!("{}{}", AVATAR_URL, map.creator_id));

        Ok(Self {
            title,
            url: format!("{}b/{}", OSU_BASE, map.beatmap_id),
            footer,
            thumbnail: ImageSource::url(format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id))
                .unwrap(),
            image: ImageSource::url(format!(
                "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
                map.beatmapset_id
            ))
            .unwrap(),
            grade_completion_mods,
            stars,
            score: unchoked_score.score as u64,
            mode: map.mode,
            acc,
            pp,
            combo,
            hits,
            map_info: osu::get_map_info(map),
            removed_misses: misses,
            prev_hits,
            prev_combo,
            prev_pp,
        })
    }
}

impl EmbedData for SimulateEmbed {
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn image(&self) -> Option<&ImageSource> {
        Some(&self.image)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        let combo = if let Some(prev_combo) = self.prev_combo {
            format!("{} → {}", prev_combo, self.combo)
        } else {
            self.combo.to_owned()
        };

        let mut fields = vec![
            ("Grade".to_owned(), self.grade_completion_mods.clone(), true),
            ("Acc".to_owned(), format!("{}%", self.acc), true),
            ("Combo".to_owned(), combo, true),
        ];

        let pp = if let Some(prev_pp) = self.prev_pp {
            format!("{} → {}", prev_pp, self.pp)
        } else {
            self.pp.to_owned()
        };

        if self.mode == GameMode::MNA {
            fields.push(("PP".to_owned(), pp, true));
            fields.push(("Score".to_owned(), with_comma_u64(self.score), true));
        } else {
            fields.push(("PP".to_owned(), pp, true));
            let hits = if let Some(ref prev_hits) = self.prev_hits {
                format!("{} → {}", prev_hits, &self.hits)
            } else {
                self.hits.to_owned()
            };
            fields.push(("Hits".to_owned(), hits, true));
        }

        fields.push(("Map Info".to_owned(), self.map_info.clone(), false));

        Some(fields)
    }

    fn minimize(self) -> EmbedBuilder {
        let mut value = if let Some(prev_pp) = self.prev_pp {
            format!("{} → {}", prev_pp, self.pp)
        } else {
            self.pp.to_string()
        };

        if self.mode != GameMode::MNA {
            let _ = write!(value, " {}", self.hits);
        }

        if let Some(misses) = self.removed_misses.filter(|misses| *misses > 0) {
            let _ = write!(value, " (+{}miss)", misses);
        }

        let combo = if self.mode == GameMode::MNA {
            String::new()
        } else if let Some(prev_combo) = self.prev_combo {
            format!(" [ {} → {} ]", prev_combo, self.combo)
        } else {
            format!(" [ {} ]", self.combo)
        };

        let score = if self.mode == GameMode::MNA {
            with_comma_u64(self.score) + " "
        } else {
            String::new()
        };

        let name = format!(
            "{grade} {score}({acc}%){combo}",
            grade = self.grade_completion_mods,
            score = score,
            acc = self.acc,
            combo = combo
        );

        EmbedBuilder::new()
            .color(DARK_GREEN)
            .unwrap()
            .field(EmbedField {
                name,
                value,
                inline: false,
            })
            .thumbnail(self.thumbnail)
            .url(self.url)
            .title(format!("{} [{}★]", self.title, self.stars))
            .unwrap()
    }
}

fn simulate_score(
    score: &mut Score,
    map: &Beatmap,
    args: SimulateArgs,
    mut attributes: StarResult,
) -> StarResult {
    match attributes {
        StarResult::Osu(diff_attributes) => {
            let mut acc = args.acc.unwrap_or_else(|| {
                let acc = score.accuracy(map.mode);

                if acc.is_nan() {
                    100.0
                } else {
                    acc
                }
            });

            acc /= 100.0;

            let mut n50 = args.n50.unwrap_or(0);
            let mut n100 = args.n100.unwrap_or(0);
            let miss = args.miss.unwrap_or(0);
            let n_objects = map.count_objects();
            let combo = args.combo.unwrap_or(diff_attributes.max_combo as u32);

            if n50 > 0 || n100 > 0 {
                let placed_points = 2 * n100 + n50 + miss;
                let missing_objects = n_objects - n100 - n50 - miss;
                let missing_points =
                    ((6.0 * acc * n_objects as f32).round() as u32).saturating_sub(placed_points);

                let mut n300 = missing_objects.min(missing_points / 6);
                n50 += missing_objects - n300;

                if let Some(orig_n50) = args.n50.filter(|_| args.n100.is_none()) {
                    // Only n50s were changed, try to load some off again onto n100s
                    let difference = n50 - orig_n50;
                    let n = n300.min(difference / 4);

                    n300 -= n;
                    n100 += 5 * n;
                    n50 -= 4 * n;
                }

                score.count300 = n300;
                score.count100 = n100;
                score.count50 = n50;
            } else {
                let target_total = (acc * n_objects as f32 * 6.0).round() as u32;
                let delta = target_total - (n_objects - miss);

                let mut n300 = delta / 5;
                let mut n100 = delta % 5;
                let mut n50 = n_objects - n300 - n100 - miss;

                // Sacrifice n300s to transform n50s into n100s
                let n = n300.min(n50 / 4);
                n300 -= n;
                n100 += 5 * n;
                n50 -= 4 * n;

                score.count300 = n300;
                score.count100 = n100;
                score.count50 = n50;
            }
            score.count_miss = miss;
            score.max_combo = combo;
            score.recalculate_grade(GameMode::STD, None);

            attributes = StarResult::Osu(diff_attributes);
        }
        StarResult::Mania(diff_attributes) => {
            let mut max_score = 1_000_000;

            let mods = score.enabled_mods;

            if mods.contains(GameMods::Easy) {
                max_score /= 2;
            }

            if mods.contains(GameMods::NoFail) {
                max_score /= 2;
            }

            if mods.contains(GameMods::HalfTime) {
                max_score /= 2;
            }

            score.max_combo = 0;
            score.score = args.score.map_or(max_score, |s| s.min(max_score));
            score.count_geki = map.count_objects();
            score.count300 = 0;
            score.count_katu = 0;
            score.count100 = 0;
            score.count50 = 0;
            score.count_miss = 0;
            score.grade = if mods.intersects(GameMods::Flashlight | GameMods::Hidden) {
                if score.score == max_score {
                    Grade::XH
                } else {
                    Grade::SH
                }
            } else if score.score == max_score {
                Grade::X
            } else {
                Grade::S
            };

            attributes = StarResult::Mania(diff_attributes);
        }
        StarResult::Taiko(diff_attributes) => {
            let n100 = args.n100.unwrap_or(0);
            let n300 = args.n300.unwrap_or(0);
            let miss = args.miss.unwrap_or(0);
            let n_objects = map.count_circle;
            let missing = n_objects - (n300 + n100 + miss);

            match args.acc {
                Some(acc) => {
                    let target_total = (acc * n_objects as f32 * 2.0 / 100.0).round() as u32;

                    let new300 = target_total
                        .saturating_sub(2 * n300)
                        .saturating_sub(n100)
                        .saturating_sub(missing);

                    let new100 = missing - new300;

                    score.count300 = n300 + new300;
                    score.count100 = n100 + new100;
                    score.count_miss = miss;
                }
                None => {
                    score.count300 = n300 + missing;
                    score.count100 = n100;
                    score.count_miss = miss;
                }
            }

            let acc = (2 * score.count300 + score.count100) as f32
                / (2 * (score.count300 + score.count100 + score.count_miss)) as f32;

            score.max_combo = args.combo.or(map.max_combo).unwrap_or(0);
            score.recalculate_grade(GameMode::TKO, Some(acc * 100.0));

            attributes = StarResult::Taiko(diff_attributes);
        }
        StarResult::Fruits(diff_attributes) => {
            let n_tiny_droplets;
            let n_tiny_droplet_misses;
            let mut n_droplets;
            let mut n_fruits;

            let miss = diff_attributes
                .max_combo
                .min(args.miss.unwrap_or(0) as usize);

            match args.acc {
                Some(acc) => {
                    n_droplets = match args.n100 {
                        Some(n100) => diff_attributes.n_droplets.min(n100 as usize),
                        None => diff_attributes.n_droplets.saturating_sub(miss),
                    };

                    n_fruits = diff_attributes.n_fruits.saturating_sub(
                        miss.saturating_sub(diff_attributes.n_droplets.saturating_sub(n_droplets)),
                    );

                    n_tiny_droplets = match args.n50 {
                        Some(n50) => diff_attributes.n_tiny_droplets.min(n50 as usize),
                        None => ((acc / 100.0
                            * (diff_attributes.max_combo + diff_attributes.n_tiny_droplets) as f32)
                            .round() as usize)
                            .saturating_sub(n_fruits)
                            .saturating_sub(n_droplets),
                    };

                    n_tiny_droplet_misses = diff_attributes
                        .n_tiny_droplets
                        .saturating_sub(n_tiny_droplets);
                }
                None => {
                    n_droplets = diff_attributes
                        .n_droplets
                        .min(args.n100.unwrap_or(0) as usize);

                    n_fruits = diff_attributes
                        .n_fruits
                        .min(args.n300.unwrap_or(0) as usize);

                    let missing_fruits = diff_attributes.n_fruits.saturating_sub(n_fruits);
                    let missing_droplets = diff_attributes.n_droplets.saturating_sub(n_droplets);

                    n_droplets += missing_droplets.saturating_sub(miss);
                    n_fruits +=
                        missing_fruits.saturating_sub(miss.saturating_sub(missing_droplets));

                    n_tiny_droplets = match args.n50 {
                        Some(n50) => diff_attributes.n_tiny_droplets.min(n50 as usize),
                        None => diff_attributes.n_tiny_droplets,
                    };

                    n_tiny_droplet_misses = diff_attributes.n_tiny_droplets - n_tiny_droplets;
                }
            }

            score.count300 = n_fruits as u32;
            score.count100 = n_droplets as u32;
            score.count50 = n_tiny_droplets as u32;
            score.count_katu = n_tiny_droplet_misses as u32;
            score.count_miss = miss as u32;
            score.max_combo = args.combo.unwrap_or(diff_attributes.max_combo as u32);
            score.recalculate_grade(GameMode::CTB, None);

            attributes = StarResult::Fruits(diff_attributes);
        }
    }

    attributes
}

fn unchoke_score(score: &mut Score, map: &Beatmap, mut attributes: StarResult) -> StarResult {
    match map.mode {
        GameMode::STD
            if score.count_miss > 0 || score.max_combo < map.max_combo.unwrap_or(5) - 5 =>
        {
            let total_objects = (map.count_circle + map.count_slider + map.count_spinner) as usize;
            let passed_objects = score.total_hits(GameMode::STD) as usize;

            let mut count300 =
                score.count300 as usize + total_objects.saturating_sub(passed_objects);

            let count_hits = total_objects - score.count_miss as usize;
            let ratio = 1.0 - (count300 as f32 / count_hits as f32);
            let new100s = (ratio * score.count_miss as f32).ceil() as u32;

            count300 += score.count_miss.saturating_sub(new100s) as usize;
            let count100 = (score.count100 + new100s) as usize;

            score.count300 = count300 as u32;
            score.count100 = count100 as u32;
            score.max_combo = map.max_combo.unwrap_or(0);
        }
        GameMode::MNA if score.grade == Grade::F => {
            score.score = 1_000_000;

            score.grade = if score
                .enabled_mods
                .intersects(GameMods::Flashlight | GameMods::Hidden)
            {
                Grade::XH
            } else {
                Grade::X
            };

            return attributes;
        }
        GameMode::CTB if score.max_combo != map.max_combo.unwrap_or(0) => {
            let diff_attributes = match attributes {
                StarResult::Fruits(attributes) => attributes,
                _ => panic!("no ctb attributes after calculating stars for ctb map"),
            };

            let total_objects = diff_attributes.max_combo;
            let passed_objects = score.total_hits(GameMode::CTB) as usize;

            let missing = total_objects - passed_objects;
            let missing_fruits = missing.saturating_sub(
                diff_attributes
                    .n_droplets
                    .saturating_sub(score.count100 as usize),
            );
            let missing_droplets = missing - missing_fruits;

            let n_fruits = score.count300 as usize + missing_fruits;
            let n_droplets = score.count100 as usize + missing_droplets;
            let n_tiny_droplet_misses = score.count_katu as usize;
            let n_tiny_droplets = diff_attributes
                .n_tiny_droplets
                .saturating_sub(n_tiny_droplet_misses);

            score.count300 = n_fruits as u32;
            score.count100 = n_droplets as u32;
            score.count_katu = n_tiny_droplet_misses as u32;
            score.count50 = n_tiny_droplets as u32;
            score.max_combo = diff_attributes.max_combo as u32;

            attributes = StarResult::Fruits(diff_attributes);
        }
        GameMode::TKO if score.grade == Grade::F || score.count_miss > 0 => {
            let total_objects = map.count_circle as usize;
            let passed_objects = score.total_hits(GameMode::TKO) as usize;

            let mut count300 =
                score.count300 as usize + total_objects.saturating_sub(passed_objects);

            let count_hits = total_objects - score.count_miss as usize;
            let ratio = 1.0 - (count300 as f32 / count_hits as f32);
            let new100s = (ratio * score.count_miss as f32).ceil() as u32;

            count300 += score.count_miss.saturating_sub(new100s) as usize;
            let count100 = (score.count100 + new100s) as usize;

            score.count300 = count300 as u32;
            score.count100 = count100 as u32;
        }
        _ => return attributes,
    }

    score.count_miss = 0;
    score.recalculate_grade(map.mode, None);

    attributes
}

fn pp(score: &Score, map: &Map, attributes: StarResult) -> PpResult {
    let mods = score.enabled_mods.bits();

    match map.mode {
        Mode::STD => OsuPP::new(&map)
            .attributes(attributes)
            .mods(mods)
            .combo(score.max_combo as usize)
            .n300(score.count300 as usize)
            .n100(score.count100 as usize)
            .n50(score.count50 as usize)
            .misses(score.count_miss as usize)
            .calculate(),
        Mode::MNA => ManiaPP::new(&map)
            .attributes(attributes)
            .mods(mods)
            .score(score.score)
            .calculate(),
        Mode::CTB => FruitsPP::new(&map)
            .attributes(attributes)
            .mods(mods)
            .combo(score.max_combo as usize)
            .fruits(score.count300 as usize)
            .droplets(score.count100 as usize)
            .misses(score.count_miss as usize)
            .accuracy(score.accuracy(GameMode::CTB))
            .calculate(),
        Mode::TKO => TaikoPP::new(&map)
            .attributes(attributes)
            .combo(score.max_combo as usize)
            .mods(mods)
            .accuracy(score.accuracy(GameMode::TKO))
            .calculate(),
    }
}
