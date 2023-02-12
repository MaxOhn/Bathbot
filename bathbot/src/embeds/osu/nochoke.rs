use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::EmbedData;
use bathbot_util::{
    constants::OSU_BASE, numbers::WithComma, AuthorBuilder, CowUtils, FooterBuilder,
};

use crate::{
    commands::osu::NochokeEntry,
    manager::redis::{osu::User, RedisData},
    pagination::Pages,
    util::{osu::grade_emote, Emote},
};

use super::ModsFormatter;

#[derive(EmbedData)]
pub struct NoChokeEmbed {
    description: String,
    title: String,
    author: AuthorBuilder,
    thumbnail: String,
    footer: FooterBuilder,
}

impl NoChokeEmbed {
    pub async fn new(
        user: &RedisData<User>,
        entries: &[NochokeEntry],
        unchoked_pp: f32,
        rank: Option<u32>,
        pages: &Pages,
    ) -> Self {
        let pp_raw = user.peek_stats(|stats| stats.pp);
        let pp_diff = (100.0 * (unchoked_pp - pp_raw)).round() / 100.0;
        let mut description = String::with_capacity(512);

        for entry in entries {
            let NochokeEntry {
                original_idx,
                original_score,
                map,
                max_pp,
                stars,
                unchoked: _,
                max_combo,
            } = entry;

            let unchoked_stats = entry.unchoked_statistics();

            let _ = writeln!(
                description,
                "**{idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars:.2}★]\n\
                {grade} {old_pp:.2} → **{new_pp:.2}pp**/{max_pp:.2}PP • ({old_acc:.2} → **{new_acc:.2}%**)\n\
                [ {old_combo} → **{new_combo}x**/{max_combo}x ]{misses}",
                idx = original_idx + 1,
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                id = map.map_id(),
                mods = ModsFormatter::new(original_score.mods),
                grade = grade_emote(entry.unchoked_grade()),
                old_pp = original_score.pp,
                new_pp = entry.unchoked_pp(),
                old_acc = original_score.accuracy,
                new_acc = entry.unchoked_accuracy(),
                old_combo = original_score.max_combo,
                new_combo = entry.unchoked_max_combo(),
                misses = MissFormat(original_score.statistics.count_miss - unchoked_stats.count_miss),
            );
        }

        let title = format!("Total pp: {pp_raw} → **{unchoked_pp}pp** (+{pp_diff})");

        let page = pages.curr_page();
        let pages = pages.last_page();
        let mut footer_text = format!("Page {page}/{pages}");

        if let Some(rank) = rank {
            let _ = write!(
                footer_text,
                " • The current rank for {pp}pp is approx. #{rank}",
                pp = WithComma::new(unchoked_pp),
                rank = WithComma::new(rank)
            );
        }

        Self {
            title,
            author: user.author_builder(),
            description,
            thumbnail: user.avatar_url().to_owned(),
            footer: FooterBuilder::new(footer_text),
        }
    }
}

struct MissFormat(u32);

impl Display for MissFormat {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.0 == 0 {
            return Ok(());
        }

        write!(
            f,
            " • *Removed {miss}{emote}*",
            miss = self.0,
            emote = Emote::Miss.text()
        )
    }
}
