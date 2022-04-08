use std::fmt::Write;

use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmapset, GameMode, Score, User};

use crate::{
    core::Context,
    embeds::osu,
    pp::PpCalculator,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        datetime::how_long_ago_dynamic,
        numbers::with_comma_int,
        osu::ScoreOrder,
        ScoreExt,
    },
};

use super::OrderAppendix;

pub struct PinnedEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl PinnedEmbed {
    pub async fn new<'i, S>(
        user: &User,
        scores: S,
        ctx: &Context,
        sort_by: ScoreOrder,
        pages: (usize, usize),
    ) -> Self
    where
        S: Iterator<Item = &'i Score>,
    {
        let mut description = String::with_capacity(512);
        let farm = HashMap::new();

        for score in scores {
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

            let mapset_opt = if let ScoreOrder::RankedDate = sort_by {
                let mapset_fut = ctx.psql().get_beatmapset::<Beatmapset>(mapset.mapset_id);

                match mapset_fut.await {
                    Ok(mapset) => Some(mapset),
                    Err(err) => {
                        let report = Report::new(err).wrap_err("failed to get mapset");
                        warn!("{report:?}");

                        None
                    }
                }
            } else {
                None
            };

            let _ = writeln!(
                description,
                "**- [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars}]\n\
                {grade} {pp} ~ {acc}% ~ {score}{appendix}\n[ {combo} ] ~ {hits} ~ {ago}",
                title = mapset.title,
                version = map.version,
                id = map.map_id,
                mods = osu::get_mods(score.mods),
                grade = score.grade_emote(score.mode),
                acc = score.acc(score.mode),
                score = with_comma_int(score.score),
                appendix = OrderAppendix::new(sort_by.into(), map, mapset_opt, score, &farm),
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
            footer: FooterBuilder::new(footer_text),
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
