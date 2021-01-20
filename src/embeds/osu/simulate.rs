use crate::{
    arguments::SimulateArgs,
    bail,
    embeds::{osu, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, DARK_GREEN, MAP_THUMB_URL, OSU_BASE},
        error::PPError,
        numbers::{round, with_comma_u64},
        osu::{grade_completion_mods, prepare_beatmap_file, simulate_score},
        ScoreExt,
    },
    BotResult,
};

use rosu::model::{Beatmap, GameMode, GameMods, Grade, Score};
use rosu_pp::{Beatmap as Map, StarResult};
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

        if is_some {
            simulate_score(&mut unchoked_score, map, args);
        } else {
            unchoke_score(&mut unchoked_score, &map).await?;
        }

        let calculations = Calculations::PP | Calculations::MAX_PP | Calculations::STARS;
        let mut calculator = PPCalculator::new().score(&unchoked_score).map(map);
        calculator.calculate(calculations).await?;

        let grade_completion_mods = grade_completion_mods(&unchoked_score, map);
        let pp = osu::get_pp(calculator.pp(), calculator.max_pp());
        let stars = round(calculator.stars().unwrap_or(0.0));
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
                    format!(
                        "**{}**/-",
                        if unchoked_score.max_combo == 0 {
                            "-".to_string()
                        } else {
                            unchoked_score.max_combo.to_string()
                        }
                    )
                } else {
                    if let Some(combo) = map.max_combo {
                        format!("**{combo}**/{combo}", combo = combo)
                    } else {
                        "**-**/-".to_string()
                    }
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

async fn unchoke_score(score: &mut Score, map: &Beatmap) -> BotResult<()> {
    let mods = score.enabled_mods.bits();

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

            return Ok(());
        }
        GameMode::CTB if score.max_combo != map.max_combo.unwrap_or(0) => {
            let map_path = prepare_beatmap_file(map.beatmap_id).await?;
            let file = File::open(map_path).map_err(PPError::from)?;
            let rosu_map = Map::parse(file).map_err(PPError::from)?;

            let attributes = match rosu_pp::fruits::stars(&rosu_map, mods, None) {
                StarResult::Fruits(attributes) => attributes,
                _ => bail!("no ctb attributes after calculating stars for ctb map"),
            };

            let total_objects = attributes.max_combo;
            let passed_objects = score.total_hits(GameMode::CTB) as usize;

            let missing = total_objects - passed_objects;
            let missing_fruits = missing.saturating_sub(
                attributes
                    .n_droplets
                    .saturating_sub(score.count100 as usize),
            );
            let missing_droplets = missing - missing_fruits;

            let n_fruits = score.count300 as usize + missing_fruits;
            let n_droplets = score.count100 as usize + missing_droplets;
            let n_tiny_droplet_misses = score.count_katu as usize;
            let n_tiny_droplets = attributes
                .n_tiny_droplets
                .saturating_sub(n_tiny_droplet_misses);

            score.count300 = n_fruits as u32;
            score.count100 = n_droplets as u32;
            score.count_katu = n_tiny_droplet_misses as u32;
            score.count50 = n_tiny_droplets as u32;
            score.max_combo = attributes.max_combo as u32;
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
        _ => return Ok(()),
    }

    score.count_miss = 0;
    score.recalculate_grade(map.mode, None);

    Ok(())
}
