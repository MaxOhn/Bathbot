use crate::{
    embeds::{osu, Author, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::OSU_BASE, datetime::how_long_ago_dynamic, numbers::with_comma_int, ScoreExt,
    },
};

use eyre::Report;
use rosu_v2::prelude::{Score, User};
use std::fmt::Write;

pub struct TopEmbed {
    author: Author,
    description: String,
    footer: Footer,
    thumbnail: String,
}

impl TopEmbed {
    pub async fn new<'i, S>(user: &User, scores: S, pages: (usize, usize)) -> Self
    where
        S: Iterator<Item = &'i (usize, Score)>,
    {
        let mut description = String::with_capacity(512);

        for (idx, score) in scores {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let mut calculator = PPCalculator::new().score(score).map(map);
            let mut calculations = Calculations::MAX_PP | Calculations::STARS;

            if score.pp.is_none() {
                calculations |= Calculations::PP;
            }

            if let Err(why) = calculator.calculate(calculations).await {
                let report = Report::new(why).wrap_err("error while calculating pp for top");
                warn!("{:?}", report);
            }

            let stars = osu::get_stars(calculator.stars().unwrap_or(0.0));
            let pp = osu::get_pp(score.pp.or_else(|| calculator.pp()), calculator.max_pp());

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ ({acc}) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                idx = idx + 1,
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

        Self {
            author: author!(user),
            description,
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
            thumbnail: user.avatar_url.to_owned(),
        }
    }
}

impl_builder!(TopEmbed {
    author,
    description,
    footer,
    thumbnail,
});
