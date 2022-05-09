use std::fmt::Write;

use command_macros::EmbedData;
use hashbrown::HashMap;
use rosu_pp::{
    Beatmap as Map, BeatmapExt, CatchPP, DifficultyAttributes, GameMode as Mode, ManiaPP, OsuPP,
    PerformanceAttributes, TaikoPP,
};
use rosu_v2::prelude::{GameMode, Grade, Score, User};

use crate::{
    core::Context,
    embeds::osu,
    error::PpError,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        datetime::how_long_ago_dynamic,
        osu::{grade_completion_mods, prepare_beatmap_file},
        CowUtils, ScoreExt,
    },
    BotResult,
};

#[derive(EmbedData)]
pub struct RecentListEmbed {
    description: String,
    thumbnail: String,
    footer: FooterBuilder,
    author: AuthorBuilder,
    title: &'static str,
}

impl RecentListEmbed {
    pub async fn new<'i, S>(
        user: &User,
        scores: S,
        ctx: &Context,
        pages: (usize, usize),
    ) -> BotResult<Self>
    where
        S: Iterator<Item = &'i Score>,
    {
        let idx = (pages.0 - 1) * 10 + 1;

        let mut mod_map = HashMap::new();
        let mut rosu_maps = HashMap::new();

        let mut description = String::with_capacity(512);

        for (score, i) in scores.zip(idx..) {
            let map = score.map.as_ref().unwrap();
            let mapset = score.mapset.as_ref().unwrap();

            #[allow(clippy::map_entry)]
            if !rosu_maps.contains_key(&map.map_id) {
                let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
                let rosu_map = Map::from_path(map_path).await.map_err(PpError::from)?;

                rosu_maps.insert(map.map_id, rosu_map);
            };

            let rosu_map = &rosu_maps[&map.map_id];

            let (pp, stars) = get_pp_stars(&mut mod_map, score, map.map_id, rosu_map);

            let _ = write!(
                description,
                "**{i}. {grade}\t[{title} [{version}]]({url})** [{stars}]",
                grade = grade_completion_mods(score, map),
                title = mapset.title.cow_escape_markdown(),
                version = map.version.cow_escape_markdown(),
                url = map.url,
            );

            if map.mode == GameMode::MNA {
                let _ = write!(description, "\t{}", osu::get_keys(score.mods, map));
            }

            description.push('\n');

            let _ = writeln!(
                description,
                "{pp}\t[ {combo} ]\t({acc}%)\t{ago}",
                combo = osu::get_combo(score, map),
                acc = score.acc(map.mode),
                ago = how_long_ago_dynamic(&score.created_at)
            );
        }

        if description.is_empty() {
            description = "No recent scores found".to_owned();
        }

        Ok(Self {
            description,
            author: author!(user),
            footer: FooterBuilder::new(format!("Page {}/{}", pages.0, pages.1)),
            thumbnail: user.avatar_url.to_owned(),
            title: "List of recent scores:",
        })
    }
}

fn get_pp_stars(
    mod_map: &mut HashMap<(u32, u32), (DifficultyAttributes, f32)>,
    score: &Score,
    map_id: u32,
    map: &Map,
) -> (String, String) {
    let bits = score.mods.bits();
    let key = (bits, map_id);

    let (mut attributes, mut max_pp) = mod_map.remove(&key).map_or_else(
        || {
            let attributes = map.stars().mods(bits).calculate();

            (attributes, None)
        },
        |(attributes, max_pp)| (attributes, Some(max_pp)),
    );

    if max_pp.is_none() {
        let result: PerformanceAttributes = match map.mode {
            Mode::STD => OsuPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate()
                .into(),
            Mode::MNA => ManiaPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate()
                .into(),
            Mode::CTB => CatchPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate()
                .into(),
            Mode::TKO => TaikoPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .calculate()
                .into(),
        };

        max_pp.replace(result.pp() as f32);
        attributes = result.into();
    }

    let max_pp = max_pp.unwrap();
    let stars = attributes.stars();
    let pp;

    if let Some(score_pp) = score.pp {
        pp = score_pp;
    } else if score.grade == Grade::F {
        let passed = score.total_hits() as usize;

        pp = match map.mode {
            Mode::STD => OsuPP::new(map)
                .mods(bits)
                .misses(score.statistics.count_miss as usize)
                .n300(score.statistics.count_300 as usize)
                .n100(score.statistics.count_100 as usize)
                .n50(score.statistics.count_50 as usize)
                .combo(score.max_combo as usize)
                .passed_objects(passed)
                .calculate()
                .pp() as f32,
            Mode::MNA => ManiaPP::new(map)
                .mods(bits)
                .score(score.score)
                .passed_objects(passed)
                .calculate()
                .pp() as f32,
            Mode::CTB => CatchPP::new(map)
                .mods(bits)
                .misses(score.statistics.count_miss as usize)
                .combo(score.max_combo as usize)
                .fruits(score.statistics.count_300 as usize)
                .droplets(score.statistics.count_100 as usize)
                .tiny_droplets(score.statistics.count_50 as usize)
                .tiny_droplet_misses(score.statistics.count_katu as usize)
                .passed_objects(passed - score.statistics.count_katu as usize)
                .calculate()
                .pp() as f32,
            Mode::TKO => TaikoPP::new(map)
                .mods(bits)
                .misses(score.statistics.count_miss as usize)
                .combo(score.max_combo as usize)
                .n300(score.statistics.count_300 as usize)
                .n100(score.statistics.count_100 as usize)
                .passed_objects(passed)
                .calculate()
                .pp() as f32,
        };
    } else {
        let result: PerformanceAttributes = match map.mode {
            Mode::STD => OsuPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .misses(score.statistics.count_miss as usize)
                .n300(score.statistics.count_300 as usize)
                .n100(score.statistics.count_100 as usize)
                .n50(score.statistics.count_50 as usize)
                .combo(score.max_combo as usize)
                .calculate()
                .into(),
            Mode::MNA => ManiaPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .score(score.score)
                .calculate()
                .into(),
            Mode::CTB => CatchPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .misses(score.statistics.count_miss as usize)
                .combo(score.max_combo as usize)
                .fruits(score.statistics.count_300 as usize)
                .droplets(score.statistics.count_100 as usize)
                .tiny_droplets(score.statistics.count_50 as usize)
                .tiny_droplet_misses(score.statistics.count_katu as usize)
                .calculate()
                .into(),
            Mode::TKO => TaikoPP::new(map)
                .mods(bits)
                .attributes(attributes)
                .misses(score.statistics.count_miss as usize)
                .combo(score.max_combo as usize)
                .n300(score.statistics.count_300 as usize)
                .n100(score.statistics.count_100 as usize)
                .calculate()
                .into(),
        };

        pp = result.pp() as f32;
        attributes = result.into();
    }

    mod_map.insert(key, (attributes, max_pp));

    let pp = format!("**{:.2}**/{:.2}PP", pp, max_pp.max(pp));
    let stars = osu::get_stars(stars as f32);

    (pp, stars)
}
