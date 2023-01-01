use std::{
    collections::hash_map::{Entry, HashMap},
    fmt::Write,
};

use bathbot_macros::EmbedData;
use rosu_pp::{BeatmapExt, DifficultyAttributes};
use rosu_v2::prelude::{GameMode, Grade, Score};

use crate::{
    core::Context,
    manager::{
        redis::{osu::User, RedisData},
        OsuMap, PpManager,
    },
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        datetime::HowLongAgoDynamic,
        hasher::IntHasher,
        osu::grade_completion_mods,
        CowUtils, ScoreExt,
    },
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

type MapId = u32;
type Mods = u32;
type AttributeMap = HashMap<(MapId, Mods), (DifficultyAttributes, f32)>;

impl RecentListEmbed {
    pub async fn new(
        user: &RedisData<User>,
        scores: &[Score],
        maps: &HashMap<u32, OsuMap, IntHasher>,
        attr_map: &mut AttributeMap,
        ctx: &Context,
        pages: &Pages,
    ) -> Self {
        let page = pages.curr_page();
        let pages = pages.last_page();

        let idx = (page - 1) * 10 + 1;

        let mut description = String::with_capacity(512);

        for (score, i) in scores.iter().zip(idx..) {
            let map = score
                .map
                .as_ref()
                .and_then(|map| maps.get(&map.map_id))
                .expect("missing map");

            let attr_key = (map.map_id(), score.mods.bits());

            let (pp, max_pp, stars) = match attr_map.entry(attr_key) {
                Entry::Occupied(entry) => {
                    let (attrs, max_pp) = entry.get();

                    let pp = if let Some(pp) = score.pp {
                        pp
                    } else if score.grade != Grade::F {
                        map.pp_map
                            .pp()
                            .attributes(attrs.to_owned())
                            .mode(PpManager::mode_conversion(score.mode))
                            .mods(score.mods.bits())
                            .state(score.state())
                            .calculate()
                            .pp() as f32
                    } else {
                        map.pp_map
                            .pp()
                            .mode(PpManager::mode_conversion(score.mode))
                            .mods(score.mods.bits())
                            .passed_objects(score.total_hits() as usize)
                            .state(score.state())
                            .calculate()
                            .pp() as f32
                    };

                    (pp, *max_pp, attrs.stars() as f32)
                }
                Entry::Vacant(entry) => {
                    let mut calc = ctx.pp(map).mode(score.mode).mods(score.mods);

                    let attrs = calc.performance().await;
                    let max_pp = attrs.pp() as f32;
                    let stars = attrs.stars() as f32;

                    let pp = match score.pp {
                        Some(pp) => pp,
                        None => calc.score(score).performance().await.pp() as f32,
                    };

                    entry.insert((attrs.into(), max_pp));

                    (pp, max_pp, stars)
                }
            };

            let _ = write!(
                description,
                "**{i}. {grade}\t[{title} [{version}]]({OSU_BASE}b/{map_id})** [{stars:.2}★]",
                grade = grade_completion_mods(score.mods, score.grade, score.total_hits(), map),
                title = map.title().cow_escape_markdown(),
                version = map.version().cow_escape_markdown(),
                map_id = map.map_id(),
            );

            if map.mode() == GameMode::Mania {
                let _ = write!(description, "\t{}", KeyFormatter::new(score.mods, map));
            }

            description.push('\n');

            let _ = writeln!(
                description,
                "{pp}\t[ {combo} ]\t({acc}%)\t{ago}",
                pp = PpFormatter::new(Some(pp), Some(max_pp)),
                combo = ComboFormatter::new(score.max_combo, map.max_combo()),
                acc = score.accuracy,
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
