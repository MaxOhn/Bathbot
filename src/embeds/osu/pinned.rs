use std::fmt::Write;

use eyre::Report;
use rosu_v2::prelude::{GameMode, Score, User};

use crate::{
    embeds::{osu, Author, Footer},
    pp::PpCalculator,
    util::{
        constants::OSU_BASE, datetime::how_long_ago_dynamic, numbers::with_comma_int, ScoreExt,
    },
};

pub struct PinnedEmbed {
    author: Author,
    description: String,
    footer: Footer,
    thumbnail: String,
}

impl PinnedEmbed {
    pub async fn new<'i, S>(user: &User, scores: S, pages: (usize, usize)) -> Self
    where
        S: Iterator<Item = &'i Score>,
    {
        let mut description = String::with_capacity(512);

        for score in scores {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let (pp, max_pp, stars) = match PpCalculator::new(map.map_id).await {
                Ok(calc) => {
                    let mut calc = calc.score(score);

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
                "**- [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ ({acc}) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                title = mapset.title,
                version = map.version,
                id = map.map_id,
                mods = osu::get_mods(score.mods),
                grade = score.grade_emote(score.mode),
                acc = score.acc_string(score.mode),
                score = with_comma_int(score.score),
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

impl_builder!(PinnedEmbed {
    author,
    description,
    footer,
    thumbnail,
});
