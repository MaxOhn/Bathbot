use crate::{
    embeds::{attachment, Author, EmbedFields, Footer},
    util::{
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::sec_to_minsec,
        error::PPError,
        numbers::{round, with_comma_uint},
        osu::{mode_emote, prepare_beatmap_file},
    },
    BotResult,
};

use chrono::{DateTime, Utc};
use rosu_pp::{Beatmap as Map, BeatmapExt, FruitsPP, GameMode as Mode, ManiaPP, OsuPP, TaikoPP};
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMode, GameMods};
use std::fmt::Write;
use tokio::fs::File;

pub struct MapEmbed {
    title: String,
    url: String,
    thumbnail: String,
    description: String,
    footer: Footer,
    author: Author,
    image: String,
    timestamp: DateTime<Utc>,
    fields: EmbedFields,
}

impl MapEmbed {
    pub async fn new(
        map: &Beatmap,
        mapset: &Beatmapset,
        mods: GameMods,
        with_thumbnail: bool,
        pages: (usize, usize),
    ) -> BotResult<Self> {
        let mut title = String::with_capacity(32);

        if map.mode == GameMode::MNA {
            let _ = write!(title, "[{}K] ", map.cs as u32);
        }

        let _ = write!(title, "{} - {}", mapset.artist, mapset.title);

        let download_value = format!(
            "[Mapset]({base}d/{mapset_id})\n\
            [No Video]({base}d/{mapset_id}n)\n\
            [Beatconnect](https://beatconnect.io/b/{mapset_id})\n\
            <osu://dl/{mapset_id}>",
            base = OSU_BASE,
            mapset_id = map.mapset_id
        );

        let mut seconds_total = map.seconds_total;
        let mut seconds_drain = map.seconds_drain;
        let mut bpm = map.bpm;

        if mods.contains(GameMods::DoubleTime) {
            seconds_total = (seconds_total as f32 * 2.0 / 3.0) as u32;
            seconds_drain = (seconds_drain as f32 * 2.0 / 3.0) as u32;
            bpm *= 1.5;
        } else if mods.contains(GameMods::HalfTime) {
            seconds_total = (seconds_total as f32 * 4.0 / 3.0) as u32;
            seconds_drain = (seconds_drain as f32 * 4.0 / 3.0) as u32;
            bpm *= 0.75;
        }

        let mut info_value = String::with_capacity(128);
        let mut fields = Vec::with_capacity(3);

        let map_path = prepare_beatmap_file(map.map_id).await?;
        let file = File::open(map_path).await.map_err(PPError::from)?;
        let rosu_map = Map::parse(file).await.map_err(PPError::from)?;
        let mod_bits = mods.bits();

        let mod_mult = 0.5_f32.powi(
            mods.contains(GameMods::Easy) as i32
                + mods.contains(GameMods::NoFail) as i32
                + mods.contains(GameMods::HalfTime) as i32,
        );

        let attributes = rosu_map.attributes().mods(mod_bits);
        let ar = attributes.ar;
        let hp = attributes.hp;
        let cs = attributes.cs;

        let od = if mods.contains(GameMods::HardRock) {
            (attributes.od * 1.4).min(10.0)
        } else if mods.contains(GameMods::Easy) {
            attributes.od * 0.5
        } else {
            attributes.od
        };

        let mut attributes = rosu_map.stars(mod_bits, None);
        let stars = attributes.stars();
        let mut pps = Vec::with_capacity(4);

        for acc in [95.0, 97.0, 99.0, 100.0].iter().copied() {
            let pp_result = match rosu_map.mode {
                Mode::STD => OsuPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(attributes)
                    .accuracy(acc)
                    .calculate(),
                Mode::MNA => ManiaPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(attributes)
                    .score(acc_to_score(mod_mult, acc) as u32)
                    .calculate(),
                Mode::CTB => FruitsPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(attributes)
                    .accuracy(acc)
                    .calculate(),
                Mode::TKO => TaikoPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(attributes)
                    .accuracy(acc)
                    .calculate(),
            };

            pps.push(pp_result.pp());
            attributes = pp_result.attributes;
        }

        let mut pp_values = String::with_capacity(128);

        let len = if rosu_map.mode == Mode::MNA {
            let len = 9.max(2 + format!("{:.2}", pps[3]).len());
            pp_values.push_str("```\n");

            let _ = writeln!(
                pp_values,
                "    |{:^len$}|{:^len$}|{:^len$}|{:^len$}",
                with_comma_uint(acc_to_score(mod_mult, 95.0)).to_string(),
                with_comma_uint(acc_to_score(mod_mult, 97.0)).to_string(),
                with_comma_uint(acc_to_score(mod_mult, 99.0)).to_string(),
                with_comma_uint(acc_to_score(mod_mult, 100.0)).to_string(),
                len = len,
            );

            len
        } else {
            let len = 6.max(2 + format!("{:.2}", pps[3]).len());
            pp_values.push_str("```\n");

            let _ = writeln!(
                pp_values,
                "Acc |{:^len$}|{:^len$}|{:^len$}|{:^len$}",
                "95%",
                "97%",
                "99%",
                "100%",
                len = len,
            );

            len
        };

        let _ = writeln!(
            pp_values,
            "----+{:->len$}+{:->len$}+{:->len$}+{:->len$}",
            "-",
            "-",
            "-",
            "-",
            len = len,
        );

        let _ = writeln!(
            pp_values,
            " PP |{:^len$}|{:^len$}|{:^len$}|{:^len$}",
            round(pps[0]),
            round(pps[1]),
            round(pps[2]),
            round(pps[3]),
            len = len
        );

        pp_values.push_str("```");

        if let Some(combo) = map.max_combo {
            let _ = write!(info_value, "Combo: `{}x`", combo);
        }

        let _ = writeln!(info_value, " Stars: `{:.2}★`", stars);

        let _ = write!(
            info_value,
            "Length: `{}` (`{}`) BPM: `{}` Objects: `{}`\n\
            CS: `{}` AR: `{}` OD: `{}` HP: `{}` Spinners: `{}`",
            sec_to_minsec(seconds_total),
            sec_to_minsec(seconds_drain),
            round(bpm),
            map.count_objects(),
            round(cs),
            round(ar),
            round(od),
            round(hp),
            map.count_spinners,
        );

        let mut info_name = format!("{} __[{}]__", mode_emote(map.mode), map.version);

        if !mods.is_empty() {
            let _ = write!(info_name, " +{}", mods);
        }

        fields.push(field!(info_name, info_value, true));
        fields.push(field!("Download", download_value, true));

        let mut field_name = format!(
            ":heart: {}  :play_pause: {}  | {:?}, {:?}",
            with_comma_uint(mapset.favourite_count),
            with_comma_uint(mapset.playcount),
            mapset.language.expect("no language in mapset"),
            mapset.genre.expect("no genre in mapset"),
        );

        if mapset.nsfw {
            field_name.push_str(" :underage: NSFW");
        }

        fields.push(field!(field_name, pp_values, false));

        let (date_text, timestamp) = if let Some(ranked_date) = mapset.ranked_date {
            (format!("{:?}", map.status), ranked_date)
        } else {
            ("Last updated".to_owned(), map.last_updated)
        };

        let author = Author::new(format!("Created by {}", mapset.creator_name))
            .url(format!("{}u/{}", OSU_BASE, mapset.creator_id))
            .icon_url(format!("{}{}", AVATAR_URL, mapset.creator_id));

        let footer_text = format!(
            "Map {} out of {} in the mapset, {}",
            pages.0, pages.1, date_text
        );

        let footer = Footer::new(footer_text);

        let thumbnail = with_thumbnail
            .then(|| format!("{}{}l.jpg", MAP_THUMB_URL, map.mapset_id))
            .unwrap_or_default();

        let image = (!with_thumbnail)
            .then(|| attachment("map_graph.png"))
            .unwrap_or_default();

        let description = format!(
            ":musical_note: [Song preview](https://b.ppy.sh/preview/{}.mp3)",
            mapset.mapset_id
        );

        Ok(Self {
            title,
            image,
            footer,
            fields,
            author,
            thumbnail,
            timestamp,
            description,
            url: format!("{}b/{}", OSU_BASE, map.map_id),
        })
    }
}

#[inline]
fn acc_to_score(mod_mult: f32, acc: f32) -> u64 {
    (mod_mult * (acc * 10_000.0 - (100.0 - acc) * 50_000.0)).round() as u64
}

impl_builder!(MapEmbed {
    author,
    description,
    fields,
    footer,
    image,
    thumbnail,
    timestamp,
    title,
    url,
});
