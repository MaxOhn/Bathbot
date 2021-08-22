use crate::{
    embeds::{osu, Author, Footer},
    util::{
        constants::OSU_BASE, datetime::how_long_ago_dynamic, numbers::with_comma_uint, ScoreExt,
    },
};

use rosu_v2::prelude::{GameMode, Score, User};
use std::fmt::Write;

pub struct TopIfEmbed {
    author: Author,
    description: String,
    footer: Footer,
    thumbnail: String,
    title: String,
}

impl TopIfEmbed {
    pub async fn new<'i, S>(
        user: &User,
        scores_data: S,
        mode: GameMode,
        pre_pp: f32,
        post_pp: f32,
        pages: (usize, usize),
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score, Option<f32>)>,
    {
        let pp_diff = (100.0 * (post_pp - pre_pp)).round() / 100.0;
        let mut description = String::with_capacity(512);

        for (idx, score, max_pp) in scores_data {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            let stars = osu::get_stars(map.stars);

            let pp = match max_pp {
                Some(max_pp) => osu::get_pp(score.pp, Some(*max_pp)),
                None => osu::get_pp(None, None),
            };

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ ({acc}) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                idx = idx,
                title = mapset.title,
                version = map.version,
                base = OSU_BASE,
                id = map.map_id,
                mods = osu::get_mods(score.mods),
                stars = stars,
                grade = score.grade_emote(mode),
                pp = pp,
                acc = score.acc_string(mode),
                score = with_comma_uint(score.score),
                combo = osu::get_combo(score, map),
                hits = score.hits_string(mode),
                ago = how_long_ago_dynamic(&score.created_at)
            );
        }

        description.pop();

        Self {
            author: author!(user),
            description,
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
            thumbnail: user.avatar_url.to_owned(),
            title: format!("Total pp: {} → **{}pp** ({:+})", pre_pp, post_pp, pp_diff),
        }
    }
}

impl_builder!(TopIfEmbed {
    author,
    description,
    footer,
    thumbnail,
    title,
});
