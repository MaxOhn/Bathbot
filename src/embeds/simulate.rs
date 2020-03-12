use super::util;
use crate::{
    arguments::SimulateArgs,
    util::{
        globals::{AVATAR_URL, HOMEPAGE, MAP_THUMB_URL},
        numbers::{round, with_comma_u64},
        osu,
        pp::PPProvider,
        Error,
    },
};

use rosu::models::{Beatmap, GameMode, Score};
use serenity::{builder::CreateEmbed, prelude::Context, utils::Colour};

pub struct SimulateData {
    pub title: String,
    pub title_url: String,
    pub stars: String,
    pub grade_completion_mods: String,
    pub acc: String,
    pub prev_pp: Option<String>,
    pub pp: String,
    pub prev_combo: Option<String>,
    pub score: Option<String>,
    pub combo: String,
    pub prev_hits: Option<String>,
    pub hits: String,
    pub removed_misses: Option<u32>,
    pub image: String,
    pub map_info: String,
    pub footer_url: String,
    pub footer_text: String,
    pub thumbnail: String,
}

impl SimulateData {
    pub fn build<'d, 'e>(&'d self, embed: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        let pp = if let Some(prev_pp) = &self.prev_pp {
            format!("{} → {}", prev_pp, self.pp)
        } else {
            self.pp.to_owned()
        };
        let combo = if let Some(prev_combo) = &self.prev_combo {
            format!("{} → {}", prev_combo, self.combo)
        } else {
            self.combo.to_owned()
        };
        let hits = if let Some(prev_hits) = &self.prev_hits {
            format!("{} → {}", prev_hits, self.hits,)
        } else {
            self.hits.to_owned()
        };
        embed
            .color(Colour::DARK_GREEN)
            .title(&self.title)
            .url(&self.title_url)
            .image(&self.image)
            .footer(|f| f.icon_url(&self.footer_url).text(&self.footer_text))
            .fields(vec![
                ("Grade", &self.grade_completion_mods, true),
                ("Acc", &self.acc, true),
                ("Combo", &combo, true),
            ]);
        if let Some(score) = &self.score {
            embed.field("PP", &pp, true).field("Score", &score, true);
        } else {
            embed.field("PP", &pp, false);
        }
        embed
            .field("Hits", &hits, false)
            .field("Map Info", &self.map_info, false)
    }

    pub fn minimize<'d, 'e>(&'d self, embed: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        let pp = if let Some(prev_pp) = &self.prev_pp {
            format!("{} → {}", prev_pp, self.pp)
        } else {
            self.pp.clone()
        };
        let combo = if let Some(prev_combo) = &self.prev_combo {
            format!("{} → {}", prev_combo, self.combo)
        } else {
            self.combo.clone()
        };
        let title = format!("{} [{}]", self.title, self.stars);
        let score = if let Some(score) = &self.score {
            format!("{} ", score)
        } else {
            String::new()
        };
        let name = format!(
            "{grade} {score}({acc}) [ {combo} ]",
            grade = self.grade_completion_mods,
            score = score,
            acc = self.acc,
            combo = combo
        );
        let mut value = format!("{} {}", pp, self.hits);
        if let Some(misses) = self.removed_misses {
            if misses > 0 {
                value.push_str(&format!(" (+{}miss)", misses));
            }
        }
        embed
            .color(Colour::DARK_GREEN)
            .field(name, value, false)
            .thumbnail(&self.thumbnail)
            .url(&self.title_url)
            .title(title)
    }

    pub fn new(
        score: Option<Score>,
        map: Beatmap,
        args: SimulateArgs,
        ctx: &Context,
    ) -> Result<Self, Error> {
        let is_some = args.is_some();
        if map.mode == GameMode::CTB || (!is_some && map.mode == GameMode::TKO) {
            return Err(Error::Custom(format!(
                "Can only simulate STD and MNA scores, not {:?}",
                map.mode,
            )));
        }
        let title = map.to_string();
        let title_url = format!("{}b/{}", HOMEPAGE, map.beatmap_id);
        let (prev_pp, prev_combo, prev_hits, misses, pp_provider) = if let Some(s) = score.as_ref()
        {
            let pp_provider = match PPProvider::new(&s, &map, Some(ctx)) {
                Ok(provider) => provider,
                Err(why) => {
                    return Err(Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    )))
                }
            };
            let prev_pp = Some(round(pp_provider.pp()).to_string());
            let prev_combo = if map.mode == GameMode::STD {
                Some(s.max_combo.to_string())
            } else {
                None
            };
            let prev_hits = Some(util::get_hits(&s, map.mode));
            (
                prev_pp,
                prev_combo,
                prev_hits,
                Some(s.count_miss),
                Some(pp_provider),
            )
        } else {
            (None, None, None, None, None)
        };
        let mut unchoked_score = score.unwrap_or_default();
        if is_some {
            osu::simulate_score(&mut unchoked_score, &map, args);
        } else {
            osu::unchoke_score(&mut unchoked_score, &map);
        }
        let pp_provider = if let Some(mut pp_provider) = pp_provider {
            if let Err(e) = pp_provider.recalculate(&unchoked_score, map.mode) {
                return Err(Error::Custom(format!(
                    "Something went wrong while recalculating PPProvider for unchoked score: {}",
                    e
                )));
            }
            pp_provider
        } else {
            match PPProvider::new(&unchoked_score, &map, Some(ctx)) {
                Ok(provider) => provider,
                Err(why) => {
                    return Err(Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    )))
                }
            }
        };
        let stars = util::get_stars(pp_provider.stars());
        let grade_completion_mods =
            util::get_grade_completion_mods(&unchoked_score, &map, ctx.cache.clone());
        let pp = util::get_pp(&unchoked_score, &pp_provider, map.mode);
        let hits = util::get_hits(&unchoked_score, map.mode);
        let (combo, acc) = match map.mode {
            GameMode::STD => (
                util::get_combo(&unchoked_score, &map),
                util::get_acc(&unchoked_score, map.mode),
            ),
            GameMode::MNA => (String::from("**-**/-"), String::from("100%")),
            m if m == GameMode::TKO && is_some => {
                let acc = unchoked_score.accuracy(GameMode::TKO);
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
                    format!("{}%", round(acc)),
                )
            }
            _ => {
                return Err(Error::Custom(format!(
                    "Cannot prepare simulate data of GameMode::{:?} score",
                    map.mode
                )))
            }
        };
        let map_info = util::get_map_info(&map);
        let footer_url = format!("{}{}", AVATAR_URL, map.creator_id);
        let footer_text = format!("{:?} map by {}", map.approval_status, map.creator);
        let thumbnail = format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id);
        let score = if map.mode == GameMode::MNA {
            Some(with_comma_u64(unchoked_score.score as u64))
        } else {
            None
        };
        Ok(Self {
            title,
            title_url,
            stars,
            grade_completion_mods,
            acc,
            score,
            prev_pp,
            pp,
            prev_combo,
            combo,
            prev_hits,
            hits,
            removed_misses: misses,
            image: format!(
                "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
                map.beatmapset_id
            ),
            map_info,
            footer_url,
            footer_text,
            thumbnail,
        })
    }
}
