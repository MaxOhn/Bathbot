use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    unwind_error,
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        ScoreExt,
    },
};

use rosu::model::{Beatmap, GameMode, Score, User};
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct NoChokeEmbed {
    description: String,
    title: String,
    author: Author,
    thumbnail: ImageSource,
    footer: Footer,
}

impl NoChokeEmbed {
    pub async fn new<'i, S>(
        user: &User,
        scores_data: S,
        unchoked_pp: f64,
        pages: (usize, usize),
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, Score, Score, Beatmap)>,
    {
        let pp_diff = (100.0 * (unchoked_pp - user.pp_raw as f64)).round() / 100.0;
        let mut description = String::with_capacity(512);
        for (idx, original, unchoked, map) in scores_data {
            let calculations = Calculations::MAX_PP | Calculations::STARS;
            let mut calculator = PPCalculator::new().score(original).map(map);
            if let Err(why) = calculator.calculate(calculations, None).await {
                unwind_error!(warn, why, "Error while calculating pp for nochokes: {}");
            }
            let stars = osu::get_stars(calculator.stars().unwrap_or(0.0));
            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {grade} {old_pp:.2} → **{new_pp:.2}pp**/{max_pp:.2}PP ~ ({old_acc:.2} → **{new_acc:.2}%**)\n\
                [ {old_combo} → **{new_combo}x**/{max_combo}x ] ~ *Removed {misses} miss{plural}*",
                idx = idx,
                title = map.title,
                version = map.version,
                base = OSU_BASE,
                id = map.beatmap_id,
                mods = osu::get_mods(original.enabled_mods),
                stars = stars,
                grade = unchoked.grade_emote(map.mode),
                old_pp = original.pp.unwrap_or(0.0),
                new_pp = unchoked.pp.unwrap_or(0.0),
                max_pp = calculator.max_pp().unwrap_or(0.0),
                old_acc = original.accuracy(GameMode::STD),
                new_acc = unchoked.accuracy(GameMode::STD),
                old_combo = original.max_combo,
                new_combo = unchoked.max_combo,
                max_combo = map.max_combo.unwrap_or(0),
                misses = original.count_miss - unchoked.count_miss,
                plural = if original.count_miss - unchoked.count_miss != 1 {
                    "es"
                } else {
                    ""
                }
            );
        }
        let title = format!(
            "Total pp: {} → **{}pp** (+{})",
            user.pp_raw, unchoked_pp, pp_diff
        );
        Self {
            title,
            author: osu::get_user_author(user),
            description,
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
        }
    }
}

impl EmbedData for NoChokeEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
}
