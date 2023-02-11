use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_util::{
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, FooterBuilder,
};
use rosu_v2::prelude::GameMode;

use crate::{
    commands::osu::TopIfEntry,
    manager::redis::{osu::User, RedisData},
    pagination::Pages,
    util::osu::grade_emote,
};

use super::{ComboFormatter, HitResultFormatter, ModsFormatter, PpFormatter};

#[derive(EmbedData)]
pub struct TopIfEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
    title: String,
}

impl TopIfEmbed {
    pub async fn new(
        user: &RedisData<User>,
        entries: &[TopIfEntry],
        mode: GameMode,
        pre_pp: f32,
        post_pp: f32,
        rank: Option<u32>,
        pages: &Pages,
    ) -> Self {
        let pp_diff = (100.0 * (post_pp - pre_pp)).round() / 100.0;
        let mut description = String::with_capacity(512);

        for entry in entries {
            let TopIfEntry {
                original_idx,
                score,
                old_pp,
                map,
                stars,
                max_pp,
            } = entry;

            let _ = writeln!(
                description,
                "**{original_idx}. [{title} [{version}]]({OSU_BASE}b/{id}) {mods}** [{stars:.2}★]\n\
                {grade} {old_pp:.2} → {pp} • {acc}% • {score}\n[ {combo} ] • {hits} • {ago}",
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                id = map.map_id(),
                mods = ModsFormatter::new(score.mods),
                grade = grade_emote(score.grade),
                pp = PpFormatter::new(Some(score.pp), Some(*max_pp)),
                acc = round(score.accuracy),
                score = WithComma::new(score.score),
                combo = ComboFormatter::new(score.max_combo, map.max_combo()),
                hits = HitResultFormatter::new(mode, score.statistics.clone()),
                ago = HowLongAgoDynamic::new(&score.ended_at)
            );
        }

        description.pop();

        let mut footer_text = format!("Page {}/{}", pages.curr_page(), pages.last_page());

        if let Some(rank) = rank {
            let _ = write!(
                footer_text,
                " • The current rank for {pp}pp is approx. #{rank}",
                pp = WithComma::new(post_pp),
                rank = WithComma::new(rank)
            );
        }

        Self {
            author: user.author_builder(),
            description,
            footer: FooterBuilder::new(footer_text),
            thumbnail: user.avatar_url().to_owned(),
            title: format!("Total pp: {pre_pp} → **{post_pp}pp** ({pp_diff:+})"),
        }
    }
}
