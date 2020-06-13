use crate::{
    arguments::SimulateArgs,
    embeds::{osu, EmbedData, Footer},
    util::{
        discord::CacheData,
        globals::{AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
        numbers::{round, with_comma_u64},
        osu::{simulate_score, unchoke_score},
        pp::PPProvider,
    },
    Error,
};

use rosu::models::{Beatmap, GameMode, GameMods, Score};
use serenity::{builder::CreateEmbed, utils::Colour};
use std::fmt::Write;

#[derive(Clone)]
pub struct SimulateEmbed {
    title: String,
    url: String,
    footer: Footer,
    thumbnail: String,
    image: String,

    stars: f32,
    grade_completion_mods: String,
    acc: f32,
    prev_pp: Option<f32>,
    pp: String,
    prev_combo: Option<u32>,
    score: Option<u64>,
    combo: String,
    prev_hits: Option<String>,
    hits: String,
    removed_misses: Option<u32>,
    map_info: String,
}

impl SimulateEmbed {
    pub async fn new<D>(
        score: Option<Score>,
        map: Beatmap,
        args: SimulateArgs,
        cache_data: D,
    ) -> Result<Self, Error>
    where
        D: CacheData,
    {
        let is_some = args.is_some();
        let title = if map.mode == GameMode::MNA {
            format!("{} {}", osu::get_keys(GameMods::default(), &map), map)
        } else {
            map.to_string()
        };
        let (prev_pp, prev_combo, prev_hits, misses) = if let Some(s) = score.as_ref() {
            let pp_provider = match PPProvider::new(&s, &map, Some(cache_data.data())).await {
                Ok(provider) => provider,
                Err(why) => {
                    return Err(Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    )))
                }
            };
            let prev_pp = Some(round(pp_provider.pp()));
            let prev_combo = if map.mode == GameMode::STD {
                Some(s.max_combo)
            } else {
                None
            };
            let prev_hits = Some(osu::get_hits(&s, map.mode));
            (prev_pp, prev_combo, prev_hits, Some(s.count_miss))
        } else {
            (None, None, None, None)
        };
        let mut unchoked_score = score.unwrap_or_default();
        if is_some {
            simulate_score(&mut unchoked_score, &map, args);
        } else {
            unchoke_score(&mut unchoked_score, &map);
        }
        let grade_completion_mods =
            osu::get_grade_completion_mods(&unchoked_score, &map, cache_data.cache()).await;
        let pp_provider =
            match PPProvider::new(&unchoked_score, &map, Some(cache_data.data())).await {
                Ok(provider) => provider,
                Err(why) => {
                    return Err(Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    )))
                }
            };
        let pp = osu::get_pp(&unchoked_score, &pp_provider);
        let hits = osu::get_hits(&unchoked_score, map.mode);
        let (combo, acc) = match map.mode {
            GameMode::STD => (
                osu::get_combo(&unchoked_score, &map),
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
            _ => {
                return Err(Error::Custom(format!(
                    "Cannot prepare simulate data of GameMode::{:?} score",
                    map.mode
                )))
            }
        };
        let footer = Footer::new(format!("{:?} map by {}", map.approval_status, map.creator))
            .icon_url(format!("{}{}", AVATAR_URL, map.creator_id));
        let score = if map.mode == GameMode::MNA {
            Some(unchoked_score.score as u64)
        } else {
            None
        };
        Ok(Self {
            title,
            url: format!("{}b/{}", HOMEPAGE, map.beatmap_id),
            footer,
            thumbnail: format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id),
            image: format!(
                "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
                map.beatmapset_id
            ),

            grade_completion_mods,
            stars: round(pp_provider.stars()),
            score,
            acc,
            pp,
            combo,
            hits,
            map_info: osu::get_map_info(&map),
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
    fn image(&self) -> Option<&str> {
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
        if let Some(score) = self.score {
            fields.push(("PP".to_owned(), pp, true));
            fields.push(("Score".to_owned(), with_comma_u64(score), true));
        } else {
            fields.push(("PP".to_owned(), pp, false));
        }
        let hits = if let Some(ref prev_hits) = self.prev_hits {
            format!("{} → {}", prev_hits, &self.hits)
        } else {
            self.hits.to_owned()
        };
        fields.push(("Hits".to_owned(), hits, false));
        fields.push(("Map Info".to_owned(), self.map_info.clone(), false));
        Some(fields)
    }
    fn minimize<'e>(&self, e: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        let mut value = if let Some(prev_pp) = self.prev_pp {
            format!("{} → {} {}", prev_pp, self.pp, self.hits)
        } else {
            format!("{} {}", self.pp, self.hits)
        };
        if let Some(misses) = self.removed_misses {
            if misses > 0 {
                let _ = write!(value, " (+{}miss)", misses);
            }
        }
        let combo = if let Some(prev_combo) = self.prev_combo {
            format!("{} → {}", prev_combo, self.combo)
        } else {
            self.combo.clone()
        };
        let score = self.score.map(with_comma_u64).unwrap_or_default();
        let name = format!(
            "{grade} {score}({acc}%) [ {combo} ]",
            grade = self.grade_completion_mods,
            score = score,
            acc = self.acc,
            combo = combo
        );
        e.color(Colour::DARK_GREEN)
            .field(name, value, false)
            .thumbnail(&self.thumbnail)
            .url(&self.url)
            .title(format!("{} [{}★]", self.title, self.stars))
    }
}
