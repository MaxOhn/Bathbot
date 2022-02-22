use std::{collections::BTreeMap, fmt::Write};

use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmap, User};

use crate::{
    custom_client::SnipeScore,
    embeds::{osu, Author, Footer},
    pp::PpCalculator,
    util::{
        constants::OSU_BASE,
        datetime::how_long_ago_dynamic,
        numbers::{round, with_comma_int},
    }, core::Context,
};

pub struct PlayerSnipeListEmbed {
    author: Author,
    description: String,
    footer: Footer,
    thumbnail: String,
}

impl PlayerSnipeListEmbed {
    pub async fn new(
        user: &User,
        scores: &BTreeMap<usize, SnipeScore>,
        maps: &HashMap<u32, Beatmap>,
        total: usize,
        ctx: &Context,
        pages: (usize, usize),
    ) -> Self {
        if scores.is_empty() {
            return Self {
                author: author!(user),
                thumbnail: user.avatar_url.to_owned(),
                footer: Footer::new("Page 1/1 ~ Total #1 scores: 0"),
                description: "No scores were found".to_owned(),
            };
        }

        let index = (pages.0 - 1) * 5;
        let entries = scores.range(index..index + 5);
        let mut description = String::with_capacity(1024);

        for (idx, score) in entries {
            let map = maps
                .get(&score.beatmap_id)
                .expect("missing beatmap for psl embed");

            let max_pp = match PpCalculator::new(ctx, map.map_id).await {
                Ok(mut calc) => Some(calc.mods(score.mods).max_pp() as f32),
                Err(err) => {
                    warn!("{:?}", Report::new(err));

                    None
                }
            };

            let pp = osu::get_pp(score.pp, max_pp);
            let n300 = map.count_objects() - score.count_100 - score.count_50 - score.count_miss;

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars}]\n\
                {pp} ~ ({acc}%) ~ {score}\n{{{n300}/{n100}/{n50}/{nmiss}}} ~ {ago}",
                idx = idx + 1,
                title = map.mapset.as_ref().unwrap().title,
                version = map.version,
                id = score.beatmap_id,
                mods = osu::get_mods(score.mods),
                stars = osu::get_stars(score.stars),
                acc = round(score.accuracy),
                score = with_comma_int(score.score),
                n100 = score.count_100,
                n50 = score.count_50,
                nmiss = score.count_miss,
                ago = how_long_ago_dynamic(&score.score_date)
            );
        }

        let footer = Footer::new(format!(
            "Page {}/{} ~ Total scores: {total}",
            pages.0, pages.1
        ));

        Self {
            author: author!(user),
            description,
            footer,
            thumbnail: user.avatar_url.to_owned(),
        }
    }
}

impl_builder!(PlayerSnipeListEmbed {
    author,
    description,
    footer,
    thumbnail,
});
