use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        numbers::round,
        ScoreExt,
    },
    BotResult,
};

use rosu::models::{Beatmap, GameMode, Score, User};
use std::fmt::Write;

#[derive(Clone)]
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
        unchoked_pp: f64,
        pages: (usize, usize),
    ) -> BotResult<Self>
    where
        S: Iterator<Item = &'i (usize, Score, Score, Beatmap)>,
    {
        let pp_diff = (100.0 * (unchoked_pp - user.pp_raw as f64)).round() / 100.0;
        let mut description = String::with_capacity(512);
        for (idx, original, unchoked, map) in scores_data {
            let calculations = Calculations::MAX_PP | Calculations::STARS;
            let mut calculator = PPCalculator::new().score(original).map(map);
            calculator.calculate(calculations, None).await?;
            let stars = osu::get_stars(calculator.stars().unwrap());
            let max_pp = round(calculator.max_pp().unwrap());
            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {grade} {old_pp} → **{new_pp}pp**/{max_pp}PP ~ ({old_acc} → **{new_acc}%**)\n\
                [ {old_combo} → **{new_combo}x**/{max_combo}x ] ~ *Removed {misses} miss{plural}*",
                idx = idx,
                title = map.title,
                version = map.version,
                base = OSU_BASE,
                id = map.beatmap_id,
                mods = osu::get_mods(original.enabled_mods),
                stars = stars,
                grade = unchoked.grade_emote(map.mode),
                old_pp = round(original.pp.unwrap()),
                new_pp = round(unchoked.pp.unwrap()),
                max_pp = max_pp,
                old_acc = round(original.accuracy(GameMode::STD)),
                new_acc = round(unchoked.accuracy(GameMode::STD)),
                old_combo = original.max_combo,
                new_combo = unchoked.max_combo,
                max_combo = map.max_combo.unwrap(),
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
        Ok(Self {
            title,
            author: osu::get_user_author(user),
            description,
            thumbnail: format!("{}{}", AVATAR_URL, user.user_id),
            footer: Footer::new(format!("Page {}/{}", pages.0, pages.1)),
        })
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
    fn thumbnail(&self) -> Option<&str> {
        Some(&self.thumbnail)
    }
}
