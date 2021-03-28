use crate::{
    custom_client::SnipeScore,
    embeds::{osu, Author, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        datetime::how_long_ago,
        numbers::{round, with_comma_uint},
    },
};

use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmap, User};
use std::{collections::BTreeMap, fmt::Write};

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
        pages: (usize, usize),
    ) -> Self {
        if scores.is_empty() {
            return Self {
                author: author!(user),
                thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
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

            let calculations = Calculations::MAX_PP;
            let mut calculator = PPCalculator::new().map(map).mods(score.mods);

            if let Err(why) = calculator.calculate(calculations).await {
                unwind_error!(warn, why, "Error while calculating pp for psl: {}");
            }

            let pp = osu::get_pp(score.pp, calculator.max_pp());
            let count_300 =
                map.count_objects() - score.count_100 - score.count_50 - score.count_miss;

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {pp} ~ ({acc}%) ~ {score}\n{{{n300}/{n100}/{n50}/{nmiss}}} ~ {ago}",
                idx = idx + 1,
                title = map.mapset.as_ref().unwrap().title,
                version = map.version,
                base = OSU_BASE,
                id = score.beatmap_id,
                mods = osu::get_mods(score.mods),
                stars = osu::get_stars(score.stars),
                pp = pp,
                acc = round(score.accuracy),
                score = with_comma_uint(score.score),
                n300 = count_300,
                n100 = score.count_100,
                n50 = score.count_50,
                nmiss = score.count_miss,
                ago = how_long_ago(&score.score_date)
            );
        }

        let footer = Footer::new(format!(
            "Page {}/{} ~ Total scores: {}",
            pages.0, pages.1, total
        ));

        Self {
            author: author!(user),
            description,
            footer,
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
        }
    }
}

impl_into_builder!(PlayerSnipeListEmbed {
    author,
    description,
    footer,
    thumbnail,
});
