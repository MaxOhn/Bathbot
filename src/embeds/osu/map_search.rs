use crate::{
    arguments::MapSearchArgs,
    embeds::Footer,
    util::{constants::OSU_BASE, numbers::round},
};

use rosu_v2::prelude::{Beatmapset, BeatmapsetSearchSort, GameMode};
use std::{collections::BTreeMap, fmt::Write};

pub struct MapSearchEmbed {
    title: String,
    description: String,
    footer: Footer,
}

impl MapSearchEmbed {
    pub async fn new(
        maps: &BTreeMap<usize, Beatmapset>,
        args: &MapSearchArgs,
        pages: (usize, Option<usize>),
    ) -> Self {
        let mut title = "Mapset results".to_owned();

        let non_empty_args = args.query.is_some()
            || args.mode.is_some()
            || args.status.is_some()
            || args.genre.is_some()
            || args.language.is_some()
            || args.video
            || args.storyboard
            || !args.nsfw
            || args.sort != BeatmapsetSearchSort::Relevance
            || !args.descending;

        if non_empty_args {
            title.push_str(" for `");
            let mut pushed = false;

            if let Some(ref query) = args.query {
                title.push_str(query);
                pushed = true;
            }

            if let Some(mode) = args.mode {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "mode={}", mode);
                pushed = true;
            }

            if let Some(ref status) = args.status {
                if pushed {
                    title.push(' ');
                }

                match status.status() {
                    Some(status) => {
                        let _ = write!(title, "status={:?}", status);
                    }
                    None => title.push_str("status=Any"),
                }

                pushed = true;
            }

            if let Some(genre) = args.genre {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "genre={:?}", genre);
                pushed = true;
            }

            if let Some(language) = args.language {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "language={:?}", language);
                pushed = true;
            }

            if args.video {
                if pushed {
                    title.push(' ');
                }

                title.push_str("video=true");
                pushed = true;
            }

            if args.storyboard {
                if pushed {
                    title.push(' ');
                }

                title.push_str("storyboard=true");
                pushed = true;
            }

            if !args.nsfw {
                if pushed {
                    title.push(' ');
                }

                title.push_str("nsfw=false");
                pushed = true;
            }

            if args.sort != BeatmapsetSearchSort::Relevance || !args.descending {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(
                    title,
                    "sort={:?} ({})",
                    args.sort,
                    if args.descending { "desc" } else { "asc" }
                );
            }

            title.push('`');
        }

        if maps.is_empty() {
            return Self {
                title,
                footer: Footer::new("Page 1/1"),
                description: String::from("No maps found for the query"),
            };
        }

        let index = (pages.0 - 1) * 10;
        let entries = maps.range(index..index + 10);
        let mut description = String::with_capacity(512);

        for (&i, mapset) in entries {
            let mut mode = String::with_capacity(4);
            let maps = mapset.maps.as_ref().unwrap();

            if maps.iter().any(|map| map.mode == GameMode::STD) {
                mode.push_str("osu!");
            }

            if maps.iter().any(|map| map.mode == GameMode::MNA) {
                if !mode.is_empty() {
                    mode.push_str(", ");
                }

                mode.push_str("mania");
            }

            if maps.iter().any(|map| map.mode == GameMode::TKO) {
                if !mode.is_empty() {
                    mode.push_str(", ");
                }

                mode.push_str("taiko");
            }

            if maps.iter().any(|map| map.mode == GameMode::CTB) {
                if !mode.is_empty() {
                    mode.push_str(", ");
                }

                mode.push_str("ctb");
            }

            let _ = writeln!(
                description,
                "**{idx}. [{artist} - {title}]({base}s/{set_id})** [{count} map{plural}]\n\
                Creator: [{creator}]({base}u/{creator_id}) ({status:?}) ~ BPM: {bpm} ~ Mode: {mode}",
                idx = i + 1,
                artist = mapset.artist,
                title = mapset.title,
                base = OSU_BASE,
                set_id = mapset.mapset_id,
                count = maps.len(),
                plural = if maps.len() != 1 { "s" } else { "" },
                creator = mapset.creator_name,
                creator_id = mapset.creator_id,
                status = mapset.status,
                bpm = round(mapset.bpm),
                mode = mode,
            );
        }

        let mut footer_text = format!("Page {}/", pages.0);

        match pages.1 {
            Some(page) => write!(footer_text, "{}", page).unwrap(),
            None => footer_text.push('?'),
        }

        Self {
            title,
            description,
            footer: Footer::new(footer_text),
        }
    }
}

impl_builder!(MapSearchEmbed {
    description,
    footer,
    title,
});
