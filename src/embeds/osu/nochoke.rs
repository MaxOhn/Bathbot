use std::{borrow::Cow, fmt::Write};

use eyre::Report;
use rosu_v2::prelude::{Score, User};

use crate::{
    embeds::{osu, Author, Footer},
    pp::PpCalculator,
    util::{
        constants::OSU_BASE,
        numbers::{with_comma_float, with_comma_int},
        ScoreExt,
    }, core::Context,
};

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
        rank: Option<usize>,
        ctx: &Context,
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

            let (max_pp, stars) = match PpCalculator::new(ctx, map.map_id).await {
                Ok(mut calc) => {
                    calc.score(original);

                    let stars = calc.stars();
                    let max_pp = calc.max_pp();

                    (max_pp, stars as f32)
                }
                Err(err) => {
                    warn!("{:?}", Report::new(err));

                    (0.0, 0.0)
                }
            };

            let stars = osu::get_stars(stars);

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars}]\n\
                {grade} {old_pp:.2} → **{new_pp:.2}pp**/{max_pp:.2}PP ~ ({old_acc:.2} → **{new_acc:.2}%**)\n\
                [ {old_combo} → **{new_combo}x**/{max_combo} ] ~ *Removed {misses} miss{plural}*",
                title = mapset.title,
                version = map.version,
                id = map.map_id,
                mods = osu::get_mods(original.mods),
                grade = unchoked.grade_emote(original.mode),
                old_pp = original.pp.unwrap_or(0.0),
                new_pp = unchoked.pp.unwrap_or(0.0),
                old_acc = original.accuracy,
                new_acc = unchoked.accuracy,
                old_combo = original.max_combo,
                new_combo = unchoked.max_combo,
                max_combo = map.max_combo.map_or_else(|| Cow::Borrowed("-"), |combo| format!("{combo}x").into()),
                misses = original.statistics.count_miss - unchoked.statistics.count_miss,
                plural = if original.statistics.count_miss - unchoked.statistics.count_miss != 1 {
                    "es"
                } else {
                    ""
                }
            );
        }

        let title = format!("Total pp: {pp_raw} → **{unchoked_pp}pp** (+{pp_diff})");

        let mut footer_text = format!("Page {}/{}", pages.0, pages.1);

        if let Some(rank) = rank {
            let _ = write!(
                footer_text,
                " • The current rank for {pp}pp is #{rank}",
                pp = with_comma_float(unchoked_pp),
                rank = with_comma_int(rank)
            );
        }

        Self {
            title,
            author: author!(user),
            description,
            thumbnail: user.avatar_url.to_owned(),
            footer: Footer::new(footer_text),
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
