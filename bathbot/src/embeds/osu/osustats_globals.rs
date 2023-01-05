use std::{collections::BTreeMap, fmt::Write};

use bathbot_macros::EmbedData;
use bathbot_util::{
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, FooterBuilder,
};

use crate::{
    commands::osu::OsuStatsEntry,
    manager::redis::{osu::User, RedisData},
    pagination::Pages,
    util::osu::grade_emote,
};

use super::{ComboFormatter, HitResultFormatter, ModsFormatter, PpFormatter};

#[derive(EmbedData)]
pub struct OsuStatsGlobalsEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl OsuStatsGlobalsEmbed {
    pub fn new(
        user: &RedisData<User>,
        entries: &BTreeMap<usize, OsuStatsEntry>,
        total: usize,
        pages: &Pages,
    ) -> Self {
        if entries.is_empty() {
            return Self {
                author: user.author_builder(),
                thumbnail: user.avatar_url().to_owned(),
                footer: FooterBuilder::new("Page 1/1 ~ Total scores: 0"),
                description: "No scores with these parameters were found".to_owned(),
            };
        }

        let page = pages.curr_page();
        let pages = pages.last_page();
        let index = (page - 1) * 5;

        let entries = entries.range(index..index + 5);
        let mut description = String::with_capacity(1024);

        for (_, entry) in entries {
            let OsuStatsEntry {
                score,
                map,
                rank,
                stars,
                max_pp,
            } = entry;

            let grade = grade_emote(score.grade);

            let _ = writeln!(
                description,
                "**[#{rank}] [{title} [{version}]]({OSU_BASE}b/{map_id}) {mods}** [{stars:.2}★]\n\
                {grade} {pp} ~ ({acc}%) ~ {score}\n[ {combo} ] ~ {hits} ~ {ago}",
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                map_id = map.map_id(),
                mods = ModsFormatter::new(score.mods),
                pp = PpFormatter::new(Some(score.pp), Some(*max_pp)),
                acc = round(score.accuracy),
                score = WithComma::new(score.score),
                combo = ComboFormatter::new(score.max_combo, map.max_combo()),
                hits = HitResultFormatter::new(score.mode, score.statistics.clone()),
                ago = HowLongAgoDynamic::new(&score.ended_at),
            );
        }

        let footer = FooterBuilder::new(format!("Page {page}/{pages} ~ Total scores: {total}"));

        Self {
            author: user.author_builder(),
            description,
            footer,
            thumbnail: user.avatar_url().to_owned(),
        }
    }
}
