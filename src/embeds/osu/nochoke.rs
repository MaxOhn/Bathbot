use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        globals::{AVATAR_URL, HOMEPAGE},
        numbers::round,
        osu::grade_emote,
        pp::PPProvider,
    },
    Error,
};

use rosu::models::{Beatmap, GameMode, Score, User};
use serenity::cache::Cache;
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
        cache: &Cache,
    ) -> Result<Self, Error>
    where
        S: Iterator<Item = &'i (usize, Score, Score, Beatmap)>,
    {
        let pp_diff = (100.0 * (unchoked_pp - user.pp_raw as f64)).round() / 100.0;
        let mut description = String::with_capacity(512);
        for (idx, original, unchoked, map) in scores_data {
            let (stars, max_pp) = {
                let pp_provider = PPProvider::new(original, map, None).await.map_err(|why| {
                    Error::Custom(format!(
                        "Something went wrong while creating PPProvider: {}",
                        why
                    ))
                })?;
                (
                    osu::get_stars(pp_provider.stars()),
                    round(pp_provider.max_pp()),
                )
            };
            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({base}b/{id}) {mods}** [{stars}]\n\
                {grade} {old_pp} → **{new_pp}pp**/{max_pp}PP ~ ({old_acc} → **{new_acc}%**)\n\
                [ {old_combo} → **{new_combo}x**/{max_combo}x ] ~ *Removed {misses} miss{plural}*",
                idx = idx,
                title = map.title,
                version = map.version,
                base = HOMEPAGE,
                id = map.beatmap_id,
                mods = osu::get_mods(original.enabled_mods),
                stars = stars,
                grade = grade_emote(unchoked.grade, cache).await,
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
