use std::fmt::{self, Write};

use eyre::Report;
use rosu_v2::prelude::{Beatmap, GameMode, GameMods, Score, User};

use crate::{
    commands::osu::TopOrder,
    core::Context,
    embeds::{osu, Author, Footer},
    pp::PpCalculator,
    util::{
        constants::OSU_BASE,
        datetime::how_long_ago_dynamic,
        numbers::{round, with_comma_int},
        ScoreExt,
    },
};

pub struct TopEmbed {
    author: Author,
    description: String,
    footer: Footer,
    thumbnail: String,
}

impl TopEmbed {
    pub async fn new<'i, S>(
        user: &User,
        scores: S,
        ctx: &Context,
        sort_by: TopOrder,
        pages: (usize, usize),
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        let mut description = String::with_capacity(512);

        for (idx, score) in scores {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let (pp, max_pp, stars) = match PpCalculator::new(ctx, map.map_id).await {
                Ok(mut calc) => {
                    calc.score(score);

                    let stars = calc.stars();
                    let max_pp = calc.max_pp();

                    let pp = match score.pp {
                        Some(pp) => pp,
                        None => calc.pp() as f32,
                    };

                    (Some(pp), Some(max_pp as f32), stars as f32)
                }
                Err(err) => {
                    warn!("{:?}", Report::new(err));

                    (None, None, 0.0)
                }
            };

            let stars = osu::get_stars(stars);
            let pp = osu::get_pp(pp, max_pp);

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ {acc}% ~ {score}{appendix}\n[ {combo} ] ~ {hits} ~ {ago}",
                idx = idx + 1,
                title = mapset.title,
                version = map.version,
                id = map.map_id,
                mods = osu::get_mods(score.mods),
                grade = score.grade_emote(score.mode),
                acc = score.acc(score.mode),
                score = with_comma_int(score.score),
                appendix = OrderAppendix::new(sort_by, map, score),
                combo = osu::get_combo(score, map),
                hits = score.hits_string(score.mode),
                ago = how_long_ago_dynamic(&score.created_at)
            );
        }

        description.pop();

        let footer_text = format!(
            "Page {}/{} | Mode: {}",
            pages.0,
            pages.1,
            mode_str(user.mode)
        );

        Self {
            author: author!(user),
            description,
            footer: Footer::new(footer_text),
            thumbnail: user.avatar_url.to_owned(),
        }
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::STD => "osu!",
        GameMode::TKO => "Taiko",
        GameMode::CTB => "Catch",
        GameMode::MNA => "Mania",
    }
}

impl_builder!(TopEmbed {
    author,
    description,
    footer,
    thumbnail,
});

pub struct OrderAppendix<'a> {
    sort_by: TopOrder,
    map: &'a Beatmap,
    score: &'a Score,
}

impl<'a> OrderAppendix<'a> {
    pub fn new(sort_by: TopOrder, map: &'a Beatmap, score: &'a Score) -> Self {
        Self {
            sort_by,
            map,
            score,
        }
    }
}

impl fmt::Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.sort_by {
            TopOrder::Bpm => {
                let mods = self.score.mods;

                let clock_rate = if mods.contains(GameMods::DoubleTime) {
                    1.5
                } else if mods.contains(GameMods::HalfTime) {
                    0.75
                } else {
                    1.0
                };

                write!(f, " ~ `{}bpm`", round(self.map.bpm * clock_rate))
            }
            TopOrder::Length => {
                let mods = self.score.mods;

                let clock_rate = if mods.contains(GameMods::DoubleTime) {
                    1.5
                } else if mods.contains(GameMods::HalfTime) {
                    0.75
                } else {
                    1.0
                };

                let secs = (self.map.seconds_drain as f32 / clock_rate) as u32;

                write!(f, " ~ `{}:{:0>2}`", secs / 60, secs % 60)
            }
            _ => Ok(()),
        }
    }
}
