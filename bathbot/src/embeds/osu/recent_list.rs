use std::{collections::hash_map::HashMap, fmt::Write};

use bathbot_macros::EmbedData;
use bathbot_util::{
    constants::OSU_BASE, datetime::HowLongAgoDynamic, numbers::round, AuthorBuilder, CowUtils,
    FooterBuilder, IntHasher,
};
use rosu_v2::prelude::GameMode;

use crate::{
    commands::osu::RecentListEntry,
    manager::{
        redis::{osu::User, RedisData},
        OsuMap,
    },
    pagination::Pages,
    util::osu::grade_completion_mods,
};

use super::{ComboFormatter, KeyFormatter, PpFormatter};

#[derive(EmbedData)]
pub struct RecentListEmbed {
    description: String,
    thumbnail: String,
    footer: FooterBuilder,
    author: AuthorBuilder,
    title: &'static str,
}

impl RecentListEmbed {
    pub fn new(
        user: &RedisData<User>,
        entries: &[RecentListEntry],
        maps: &HashMap<u32, OsuMap, IntHasher>,
        pages: &Pages,
    ) -> Self {
        let page = pages.curr_page();
        let pages = pages.last_page();

        let mut description = String::with_capacity(512);

        for entry in entries {
            let RecentListEntry {
                idx,
                score,
                map_id,
                stars,
                max_pp,
                max_combo,
            } = entry;

            let map = maps.get(map_id).expect("missing map");

            let _ = write!(
                description,
                "**{i}. {grade}\t[{title} [{version}]]({OSU_BASE}b/{map_id})** [{stars:.2}★]",
                i = *idx + 1,
                grade = grade_completion_mods(score.mods, score.grade, score.total_hits(), map),
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                map_id = map.map_id(),
            );

            if score.mode == GameMode::Mania {
                let _ = write!(description, "\t{}", KeyFormatter::new(score.mods, map));
            }

            description.push('\n');

            let _ = writeln!(
                description,
                "{pp}\t[ {combo} ]\t({acc}%)\t{ago}",
                pp = PpFormatter::new(Some(score.pp), Some(*max_pp)),
                combo = ComboFormatter::new(score.max_combo, Some(*max_combo)),
                acc = round(score.accuracy),
                ago = HowLongAgoDynamic::new(&score.ended_at)
            );
        }

        if description.is_empty() {
            description = "No recent scores found".to_owned();
        }

        Self {
            description,
            author: user.author_builder(),
            footer: FooterBuilder::new(format!("Page {page}/{pages}")),
            thumbnail: user.avatar_url().to_owned(),
            title: "List of recent scores:",
        }
    }
}
