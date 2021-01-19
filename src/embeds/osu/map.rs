use crate::{
    embeds::{Author, EmbedData, Footer},
    util::{
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::sec_to_minsec,
        error::PPError,
        numbers::{round, with_comma_u64},
        osu::{mode_emote, prepare_beatmap_file},
    },
    BotResult,
};

use chrono::{DateTime, Utc};
use rosu::model::{Beatmap, GameMode, GameMods};
use rosu_pp::{
    osu::no_leniency, Beatmap as Map, BeatmapExt, FruitsPP, GameMode as Mode, ManiaPP, OsuPP,
    TaikoPP,
};
use std::{fmt::Write, fs::File};
use twilight_embed_builder::image_source::ImageSource;

pub struct MapEmbed {
    title: String,
    url: String,
    thumbnail: Option<ImageSource>,
    description: String,
    footer: Footer,
    author: Author,
    image: Option<ImageSource>,
    timestamp: DateTime<Utc>,
    fields: Vec<(String, String, bool)>,
}

impl MapEmbed {
    pub async fn new(
        map: &Beatmap,
        mods: GameMods,
        with_thumbnail: bool,
        pages: (usize, usize),
    ) -> BotResult<Self> {
        let mut title = String::with_capacity(32);
        if map.mode == GameMode::MNA {
            let _ = write!(title, "[{}K] ", map.diff_cs as u32);
        }
        let _ = write!(title, "{} - {}", map.artist, map.title);
        let download_value = format!(
            "[Mapset]({base}d/{mapset_id})\n\
            [No Video]({base}d/{mapset_id}n)\n\
            [Beatconnect](https://beatconnect.io/b/{mapset_id})\n\
            <osu://dl/{mapset_id}>",
            base = OSU_BASE,
            mapset_id = map.beatmapset_id
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

        let map_path = prepare_beatmap_file(map.beatmap_id).await?;
        let file = File::open(map_path).map_err(PPError::from)?;
        let rosu_map = Map::parse(file).map_err(PPError::from)?;
        let mod_bits = mods.bits();

        let mod_mult = 0.5_f32.powi(
            mods.contains(GameMods::Easy) as i32
                + mods.contains(GameMods::NoFail) as i32
                + mods.contains(GameMods::HalfTime) as i32,
        );

        let attributes = rosu_map.attributes().mods(mod_bits);
        let ar = attributes.ar;
        let od = attributes.od;
        let hp = attributes.hp;
        let cs = attributes.cs;

        let mut star_result = rosu_map.stars(mod_bits, None);
        let stars = star_result.stars();
        let mut pps = Vec::with_capacity(4);

        for acc in [95.0, 97.0, 99.0, 100.0].iter().copied() {
            let pp_result = match rosu_map.mode {
                Mode::STD => OsuPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(star_result)
                    .accuracy(acc)
                    .calculate(no_leniency::stars),
                Mode::MNA => ManiaPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(star_result)
                    .score(acc_to_score(mod_mult, acc) as u32)
                    .calculate(),
                Mode::CTB => FruitsPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(star_result)
                    .accuracy(acc)
                    .calculate(),
                Mode::TKO => TaikoPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(star_result)
                    .accuracy(acc)
                    .calculate(),
            };

            pps.push(pp_result.pp());
            star_result = pp_result.attributes;
        }

        let mut pp_values = String::with_capacity(128);

        let len = if rosu_map.mode == Mode::MNA {
            let len = 9.max(2 + format!("{:.2}", pps[3]).len());
            pp_values.push_str("```\n");

            let _ = writeln!(
                pp_values,
                "    |{:^len$}|{:^len$}|{:^len$}|{:^len$}",
                with_comma_u64(acc_to_score(mod_mult, 95.0)),
                with_comma_u64(acc_to_score(mod_mult, 97.0)),
                with_comma_u64(acc_to_score(mod_mult, 99.0)),
                with_comma_u64(acc_to_score(mod_mult, 100.0)),
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
            map.count_spinner,
        );

        let mut info_name = format!("{} __[{}]__", mode_emote(map.mode), map.version);

        if !mods.is_empty() {
            let _ = write!(info_name, " +{}", mods);
        }

        fields.push((info_name, info_value, true));
        fields.push(("Download".to_owned(), download_value, true));

        let field_name = format!(
            ":heart: {}  :play_pause: {}  | {:?}, {:?}",
            with_comma_u64(map.favourite_count as u64),
            with_comma_u64(map.playcount as u64),
            map.language,
            map.genre,
        );

        fields.push((field_name, pp_values, false));

        let (date_text, timestamp) = if let Some(approved_date) = map.approved_date {
            (format!("{:?}", map.approval_status), approved_date)
        } else {
            ("Last updated".to_owned(), map.last_update)
        };

        let author = Author::new(format!("Created by {}", map.creator))
            .url(format!("{}u/{}", OSU_BASE, map.creator_id))
            .icon_url(format!("{}{}", AVATAR_URL, map.creator_id));

        let footer_text = format!(
            "Map {} out of {} in the mapset, {}",
            pages.0, pages.1, date_text
        );

        let footer = Footer::new(footer_text);

        let thumbnail = if with_thumbnail {
            Some(ImageSource::url(format!("{}{}l.jpg", MAP_THUMB_URL, map.beatmapset_id)).unwrap())
        } else {
            None
        };

        let image = if with_thumbnail {
            None
        } else {
            Some(ImageSource::attachment("map_graph.png").unwrap())
        };

        let description = format!(
            ":musical_note: [Song preview](https://b.ppy.sh/preview/{}.mp3)",
            map.beatmapset_id
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
            url: format!("{}b/{}", OSU_BASE, map.beatmap_id),
        })
    }
}

#[inline]
fn acc_to_score(mod_mult: f32, acc: f32) -> u64 {
    (mod_mult * (acc * 10_000.0 - (100.0 - acc) * 50_000.0)).round() as u64
}

impl EmbedData for MapEmbed {
    fn thumbnail(&self) -> Option<&ImageSource> {
        self.thumbnail.as_ref()
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
    fn image(&self) -> Option<&ImageSource> {
        self.image.as_ref()
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
    fn timestamp(&self) -> Option<&DateTime<Utc>> {
        Some(&self.timestamp)
    }
}
