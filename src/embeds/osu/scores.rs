use std::fmt::{self, Write};

use command_macros::EmbedData;
use eyre::Report;
use rosu_pp::{Beatmap as Map, BeatmapExt};
use rosu_v2::prelude::{Beatmap, GameMode, Score, User};

use crate::{
    core::{Context, CONFIG},
    error::PpError,
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago_dynamic,
        numbers::{round, with_comma_int},
        osu::prepare_beatmap_file,
        CowUtils, Emote, ScoreExt,
    },
    BotResult,
};

#[derive(EmbedData)]
pub struct ScoresEmbed {
    description: String,
    thumbnail: String,
    footer: FooterBuilder,
    author: AuthorBuilder,
    title: String,
    url: String,
}

impl ScoresEmbed {
    #[allow(clippy::too_many_arguments)]
    pub async fn new<'i, S>(
        user: &User,
        map: &Beatmap,
        mut scores: S,
        pinned: &[Score],
        personal: &[Score],
        global: Option<(usize, usize)>,
        pp_idx: usize,
        pages: &Pages,
        ctx: &Context,
    ) -> Self
    where
        S: Iterator<Item = &'i Score>,
    {
        let pp_map = match get_map(ctx, map.map_id).await {
            Ok(map) => Some(map),
            Err(err) => {
                let report = Report::new(err).wrap_err("failed to prepare map for pp calculation");
                warn!("{report:?}");

                None
            }
        };

        let page = pages.curr_page();
        let pages = pages.last_page();

        let mut description = String::with_capacity(512);
        let pp_idx = (page == pp_idx / 10 + 1).then(|| pp_idx % 10);
        let mut args = WriteArgs::new(&mut description, pinned, personal, global, pp_idx);

        if page == 1 {
            if let Some(score) = scores.next() {
                let personal = personal_idx(score, args.personal);

                if personal.is_some() || matches!(args.global, Some((0, _))) {
                    args.description.push_str("__**");

                    if let Some(idx) = personal {
                        let _ = write!(args.description, "Personal Best #{idx}");
                    }

                    if let Some((_, idx)) = args.global.filter(|(idx, _)| *idx == 0) {
                        if personal.is_some() {
                            args.description.push_str(" and ");
                        }

                        let _ = write!(args.description, "Global Top #{idx}");
                    }

                    args.description.push_str("**__");
                }

                let mut pinned = args.pinned.iter();

                if pinned.any(|s| s.score_id == score.score_id && s.mods == score.mods) {
                    args.description.push_str(" 📌");
                }

                if !args.description.is_empty() {
                    args.description.push('\n');
                }

                let (pp, max_pp, stars) = get_attrs(&pp_map, score);

                let _ = writeln!(
                    args.description,
                    "{grade} **+{mods}** [{stars:.2}★] • {score} • {acc}%\n\
                    {pp_format}**{pp}**{pp_format}/{max_pp}PP • **{combo}x**/{max_combo}x\n\
                    {hits} {timestamp}",
                    grade = CONFIG.get().unwrap().grade(score.grade),
                    mods = score.mods,
                    score = with_comma_int(score.score),
                    acc = round(score.accuracy),
                    pp_format = if pp_idx == Some(0) { "" } else { "~~" },
                    pp = pp.map_or(0.0, round),
                    max_pp = OptionFormat::new(pp.zip(max_pp).map(|(pp, max)| pp.max(max))),
                    combo = score.max_combo,
                    max_combo = OptionFormat::new(map.max_combo),
                    hits = score.hits_string(score.mode),
                    timestamp = how_long_ago_dynamic(&score.created_at)
                );

                if let Some(score) = scores.next() {
                    args.description
                        .push_str("\n__Other scores on the beatmap:__\n");
                    let (pp, _, stars) = get_attrs(&pp_map, score);
                    write_compact_score(&mut args, 1, score, stars, pp.unwrap_or(0.0));
                }
            }
        }

        for (score, i) in scores.zip(2..) {
            let (pp, _, stars) = get_attrs(&pp_map, score);
            write_compact_score(&mut args, i, score, stars, pp.unwrap_or(0.0));
        }

        if args.description.is_empty() {
            args.description.push_str("No scores found");
        }

        let (artist, title, creator_name, creator_id, status) = {
            let ms = map
                .mapset
                .as_ref()
                .expect("mapset neither in map nor in option");

            (
                &ms.artist,
                &ms.title,
                &ms.creator_name,
                ms.creator_id,
                ms.status,
            )
        };

        let footer_text = format!("Page {page}/{pages} • {status:?} map by {creator_name}");
        let footer =
            FooterBuilder::new(footer_text).icon_url(format!("{AVATAR_URL}{}", creator_id));

        let mut title_text = String::with_capacity(32);

        let _ = write!(
            title_text,
            "{artist} - {title} [{version}]",
            artist = artist.cow_escape_markdown(),
            title = title.cow_escape_markdown(),
            version = map.version.cow_escape_markdown()
        );

        if map.mode == GameMode::MNA {
            let _ = write!(title_text, "[{}K] ", map.cs as u32);
        }

        Self {
            description,
            footer,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id),
            title: title_text,
            url: format!("{OSU_BASE}b/{}", map.map_id),
            author: author!(user),
        }
    }
}

async fn get_map(ctx: &Context, map_id: u32) -> BotResult<Map> {
    let map_path = prepare_beatmap_file(ctx, map_id).await?;
    let map = Map::from_path(map_path).await.map_err(PpError::from)?;

    Ok(map)
}

fn get_attrs(map: &Option<Map>, score: &Score) -> (Option<f32>, Option<f32>, f32) {
    match map {
        Some(ref map) => {
            let mods = score.mods.bits();
            let performance = map.pp().mods(mods).calculate();

            let max_pp = performance.pp() as f32;
            let stars = performance.stars() as f32;

            let pp = match score.pp {
                Some(pp) => pp,
                None => {
                    let performance = map
                        .pp()
                        .attributes(performance)
                        .mods(mods)
                        .n300(score.statistics.count_300 as usize)
                        .n100(score.statistics.count_100 as usize)
                        .n50(score.statistics.count_50 as usize)
                        .n_katu(score.statistics.count_katu as usize)
                        .score(score.score)
                        .combo(score.max_combo as usize)
                        .misses(score.statistics.count_miss as usize)
                        .calculate();

                    performance.pp() as f32
                }
            };

            (Some(pp), Some(max_pp), stars)
        }
        None => (score.pp, None, 0.0),
    }
}

struct WriteArgs<'c> {
    description: &'c mut String,
    pinned: &'c [Score],
    personal: &'c [Score],
    global: Option<(usize, usize)>,
    pp_idx: Option<usize>,
}

impl<'c> WriteArgs<'c> {
    fn new(
        description: &'c mut String,
        pinned: &'c [Score],
        personal: &'c [Score],
        global: Option<(usize, usize)>,
        pp_idx: Option<usize>,
    ) -> Self {
        Self {
            description,
            pinned,
            personal,
            global,
            pp_idx,
        }
    }
}

fn personal_idx(score: &Score, scores: &[Score]) -> Option<usize> {
    scores
        .iter()
        .position(|s| s.created_at == score.created_at)
        .map(|i| i + 1)
}

fn write_compact_score(args: &mut WriteArgs<'_>, i: usize, score: &Score, stars: f32, pp: f32) {
    let config = CONFIG.get().unwrap();

    let _ = write!(
        args.description,
        "{grade} **+{mods}** [{stars:.2}★] {pp_format}{pp:.2}pp{pp_format} \
        ({acc}%) {combo}x • {miss}{miss_emote} {timestamp}",
        grade = config.grade(score.grade),
        mods = score.mods,
        pp_format = if args.pp_idx == Some(i) { "**" } else { "~~" },
        acc = round(score.accuracy),
        combo = score.max_combo,
        miss = score.statistics.count_miss,
        miss_emote = Emote::Miss.text(),
        timestamp = how_long_ago_dynamic(&score.created_at),
    );

    let mut pinned = args.pinned.iter();

    if pinned.any(|s| s.score_id == score.score_id && s.mods == score.mods) {
        args.description.push_str(" 📌");
    }

    let personal = personal_idx(score, args.personal);

    if personal.is_some() || matches!(args.global, Some((n, _)) if n == i) {
        args.description.push_str(" **(");

        if let Some(idx) = personal {
            let _ = write!(args.description, "Personal Best #{idx}");
        }

        if let Some((_, idx)) = args.global.filter(|(idx, _)| *idx == i) {
            if personal.is_some() {
                args.description.push_str(" and ");
            }

            let _ = write!(args.description, "Global Top #{idx}");
        }

        args.description.push_str(")**");
    }

    args.description.push('\n');
}

struct OptionFormat<T> {
    value: Option<T>,
}

impl<T> OptionFormat<T> {
    fn new(value: Option<T>) -> Self {
        Self { value }
    }
}

impl<T: fmt::Display> fmt::Display for OptionFormat<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Some(ref value) => write!(f, "{value:.2}"),
            None => f.write_str("-"),
        }
    }
}
