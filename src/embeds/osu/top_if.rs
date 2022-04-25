use std::fmt::Write;

use command_macros::EmbedData;
use rosu_v2::prelude::{GameMode, Score, User};

use crate::{
    embeds::osu,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        datetime::how_long_ago_dynamic,
        numbers::{with_comma_float, with_comma_int},
        ScoreExt,
    },
};

#[derive(EmbedData)]
pub struct TopIfEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
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
        rank: Option<usize>,
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
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ {acc}% ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                title = mapset.title,
                version = map.version,
                id = map.map_id,
                mods = osu::get_mods(score.mods),
                grade = score.grade_emote(mode),
                acc = score.acc(mode),
                score = with_comma_int(score.score),
                combo = osu::get_combo(score, map),
                hits = score.hits_string(mode),
                ago = how_long_ago_dynamic(&score.created_at)
            );
        }

        description.pop();

        let mut footer_text = format!("Page {}/{}", pages.0, pages.1);

        if let Some(rank) = rank {
            let _ = write!(
                footer_text,
                " • The current rank for {pp}pp is #{rank}",
                pp = with_comma_float(post_pp),
                rank = with_comma_int(rank)
            );
        }

        Self {
            author: author!(user),
            description,
            footer: FooterBuilder::new(footer_text),
            thumbnail: user.avatar_url.to_owned(),
            title: format!("Total pp: {pre_pp} → **{post_pp}pp** ({pp_diff:+})"),
        }
    }
}