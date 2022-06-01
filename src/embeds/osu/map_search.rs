use std::{collections::BTreeMap, fmt::Write};

use command_macros::EmbedData;
use rosu_v2::prelude::{Beatmapset, GameMode, Genre, Language};

use crate::{
    commands::osu::{Search, SearchOrder},
    pagination::Pages,
    util::{builder::FooterBuilder, constants::OSU_BASE, numbers::round, CowUtils},
};

#[derive(EmbedData)]
pub struct MapSearchEmbed {
    title: String,
    description: String,
    footer: FooterBuilder,
}

impl MapSearchEmbed {
    pub fn new(maps: &BTreeMap<usize, Beatmapset>, args: &Search, pages: &Pages) -> Self {
        let mut title = "Mapset results".to_owned();
        let sort = args.sort.unwrap_or_default();

        let non_empty_args = args.query.is_some()
            || args.mode.is_some()
            || args.status.is_some()
            || args.genre.is_some()
            || args.language.is_some()
            || args.video == Some(true)
            || args.storyboard == Some(true)
            || args.nsfw == Some(false)
            || sort != SearchOrder::Relevance
            || args.reverse == Some(true);

        if non_empty_args {
            title.push_str(" for `");
            let mut pushed = false;

            if let Some(ref query) = args.query {
                title.push_str(query);
                pushed = true;
            }

            if let Some(mode) = args.mode.map(GameMode::from) {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "mode={mode}");
                pushed = true;
            }

            if let Some(ref status) = args.status {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "status={status:?}");
                pushed = true;
            }

            if let Some(genre) = args.genre.map(Genre::from) {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "genre={genre:?}");
                pushed = true;
            }

            if let Some(language) = args.language.map(Language::from) {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(title, "language={language:?}");
                pushed = true;
            }

            if args.video == Some(true) {
                if pushed {
                    title.push(' ');
                }

                title.push_str("video=true");
                pushed = true;
            }

            if args.storyboard == Some(true) {
                if pushed {
                    title.push(' ');
                }

                title.push_str("storyboard=true");
                pushed = true;
            }

            if args.nsfw == Some(false) {
                if pushed {
                    title.push(' ');
                }

                title.push_str("nsfw=false");
                pushed = true;
            }

            if args.sort != Some(SearchOrder::Relevance) || args.reverse == Some(true) {
                if pushed {
                    title.push(' ');
                }

                let _ = write!(
                    title,
                    "sort={:?} ({})",
                    sort,
                    if args.reverse == Some(true) {
                        "asc"
                    } else {
                        "desc"
                    }
                );
            }

            title.push('`');
        }

        if maps.is_empty() {
            return Self {
                title,
                footer: FooterBuilder::new("Page 1/1"),
                description: String::from("No maps found for the query"),
            };
        }

        let index = pages.index;
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
                "**{idx}. [{artist} - {title}]({OSU_BASE}s/{set_id})** [{count} map{plural}]\n\
                Creator: [{creator}]({OSU_BASE}u/{creator_id}) ({status:?}) ~ BPM: {bpm} ~ Mode: {mode}",
                idx = i + 1,
                artist = mapset.artist.cow_escape_markdown(),
                title = mapset.title.cow_escape_markdown(),
                set_id = mapset.mapset_id,
                count = maps.len(),
                plural = if maps.len() != 1 { "s" } else { "" },
                creator = mapset.creator_name.cow_escape_markdown(),
                creator_id = mapset.creator_id,
                status = mapset.status,
                bpm = round(mapset.bpm),
            );
        }

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer_text = format!("Page {page}/{pages}");

        Self {
            title,
            description,
            footer: FooterBuilder::new(footer_text),
        }
    }
}
