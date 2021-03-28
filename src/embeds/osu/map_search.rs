use crate::{
    custom_client::BeatconnectMapSet,
    embeds::Footer,
    util::{constants::OSU_BASE, numbers::round},
};

use std::{collections::BTreeMap, fmt::Write};

pub struct MapSearchEmbed {
    title: String,
    description: String,
    footer: Footer,
}

impl MapSearchEmbed {
    pub async fn new(
        maps: &BTreeMap<usize, BeatconnectMapSet>,
        query: &str,
        pages: (usize, Option<usize>),
    ) -> Self {
        let title = format!("Mapset results for `{}`", query);

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

            if mapset.mode_std {
                mode.push_str("osu!");
            }

            if mapset.mode_mania {
                if !mode.is_empty() {
                    mode.push_str(", ");
                }

                mode.push_str("mania");
            }

            if mapset.mode_taiko {
                if !mode.is_empty() {
                    mode.push_str(", ");
                }

                mode.push_str("taiko");
            }

            if mapset.mode_ctb {
                if !mode.is_empty() {
                    mode.push_str(", ");
                }

                mode.push_str("ctb");
            }

            let _ = writeln!(
                description,
                "**{idx}. [{artist} - {title}]({base}s/{set_id})** [{count} map{plural}]\n\
                Creator: [{creator}]({base}u/{creator_id}) ({status}) ~ BPM: {bpm} ~ Mode: {mode}",
                idx = i + 1,
                artist = mapset.artist,
                title = mapset.title,
                base = OSU_BASE,
                set_id = mapset.beatmapset_id,
                count = mapset.maps.len(),
                plural = if mapset.maps.len() != 1 { "s" } else { "" },
                creator = mapset.creator,
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

impl_into_builder!(MapSearchEmbed {
    description,
    footer,
    title,
});
