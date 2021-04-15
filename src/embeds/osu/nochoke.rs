use crate::{
    embeds::{osu, Author, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        ScoreExt,
    },
};

use rosu_v2::prelude::{Score, User};
use std::{borrow::Cow, fmt::Write};

pub struct NoChokeEmbed {
    description: String,
    title: String,
    author: Author,
    thumbnail: String,
    footer: Footer,
}

impl NoChokeEmbed {
    pub async fn new<'i, S>(
        user: &User,
        scores_data: S,
        unchoked_pp: f32,
        pages: (usize, usize),
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score, Score)>,
    {
        let pp_raw = user.statistics.as_ref().unwrap().pp;
        let pp_diff = (100.0 * (unchoked_pp - pp_raw as f32)).round() / 100.0;
        let mut description = String::with_capacity(512);

        for (idx, original, unchoked) in scores_data {
            let map = original.map.as_ref().unwrap();
            let mapset = original.mapset.as_ref().unwrap();

            let calculations = Calculations::MAX_PP | Calculations::STARS;
            let mut calculator = PPCalculator::new().score(original).map(map);

            if let Err(why) = calculator.calculate(calculations).await {
                unwind_error!(warn, why, "Error while calculating pp for nochokes: {}");
            }

            let stars = osu::get_stars(calculator.stars().unwrap_or(0.0));

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {grade} {old_pp:.2} → **{new_pp:.2}pp**/{max_pp:.2}PP ~ ({old_acc:.2} → **{new_acc:.2}%**)\n\
                [ {old_combo} → **{new_combo}x**/{max_combo} ] ~ *Removed {misses} miss{plural}*",
                idx = idx,
                title = mapset.title,
                version = map.version,
                base = OSU_BASE,
                id = map.map_id,
                mods = osu::get_mods(original.mods),
                stars = stars,
                grade = unchoked.grade_emote(original.mode),
                old_pp = original.pp.unwrap_or(0.0),
                new_pp = unchoked.pp.unwrap_or(0.0),
                max_pp = calculator.max_pp().unwrap_or(0.0),
                old_acc = original.accuracy,
                new_acc = unchoked.accuracy,
                old_combo = original.max_combo,
                new_combo = unchoked.max_combo,
                max_combo = map.max_combo.map_or_else(|| Cow::Borrowed("-"), |combo| format!("{}x", combo).into()),
                misses = original.statistics.count_miss - unchoked.statistics.count_miss,
                plural = if original.statistics.count_miss - unchoked.statistics.count_miss != 1 {
                    "es"
                } else {
                    ""
                }
            );
        }

        let title = format!(
            "Total pp: {} → **{}pp** (+{})",
            pp_raw, unchoked_pp, pp_diff
        );

        Self {
            title,
            author: author!(user),
            description,
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
        }
    }
}

impl_builder!(NoChokeEmbed {
    author,
    description,
    footer,
    thumbnail,
    title,
});
