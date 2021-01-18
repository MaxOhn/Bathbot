use crate::{
    arguments::SimulateArgs,
    embeds::{osu, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, DARK_GREEN, MAP_THUMB_URL, OSU_BASE},
        numbers::{round, with_comma_u64},
        osu::{grade_completion_mods, simulate_score, unchoke_score},
        ScoreExt,
    },
    BotResult,
};

use rosu::model::{Beatmap, GameMode, GameMods, Score};
use std::fmt::Write;
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
            unchoke_score(&mut unchoked_score, map);
        }
        let grade_completion_mods = grade_completion_mods(&unchoked_score, map);
        let calculations = Calculations::PP | Calculations::MAX_PP | Calculations::STARS;
        let mut calculator = PPCalculator::new().score(&unchoked_score).map(map);
        calculator.calculate(calculations).await?;
        let pp = osu::get_pp(calculator.pp(), calculator.max_pp());
        let stars = round(calculator.stars().unwrap_or(0.0));
        let hits = unchoked_score.hits_string(map.mode);
        let (combo, acc) = match map.mode {
            GameMode::STD => (
                osu::get_combo(&unchoked_score, map),
                round(unchoked_score.accuracy(map.mode)),
            ),
            GameMode::MNA => (String::from("**-**/-"), 100.0),
            m if m == GameMode::TKO && is_some => {
                let acc = round(unchoked_score.accuracy(GameMode::TKO));
                let combo = unchoked_score.max_combo;
                (
                    format!(
                        "**{}**/-",
                        if combo == 0 {
                            "-".to_string()
                        } else {
                            combo.to_string()
                        }
                    ),
                    acc,
                )
            }
            _ => panic!("Cannot prepare simulate data of {:?} score", map.mode),
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
