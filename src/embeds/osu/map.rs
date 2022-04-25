use std::fmt::Write;

use chrono::{DateTime, Utc};
use command_macros::EmbedData;
use rosu_pp::{
    Beatmap as Map, BeatmapExt, CatchPP, GameMode as Mode, ManiaPP, Mods, OsuPP,
    PerformanceAttributes, TaikoPP,
};
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMode, GameMods};

use crate::{
    commands::osu::CustomAttrs,
    core::{Context, CONFIG},
    embeds::{attachment, EmbedFields},
    error::PpError,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::{AVATAR_URL, OSU_BASE},
        datetime::sec_to_minsec,
        numbers::{round, with_comma_int},
        osu::{mode_emote, prepare_beatmap_file},
    },
    BotResult,
};

use super::{calculate_ar, calculate_od};

#[derive(EmbedData)]
pub struct MapEmbed {
    title: String,
    url: String,
    thumbnail: String,
    description: String,
    footer: FooterBuilder,
    author: AuthorBuilder,
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
        attrs: &CustomAttrs,
        ctx: &Context,
        pages: (usize, usize),
    ) -> BotResult<Self> {
        let mut title = String::with_capacity(32);

        if map.mode == GameMode::MNA {
            let _ = write!(title, "[{}K] ", map.cs as u32);
        }

        let _ = write!(title, "{} - {}", mapset.artist, mapset.title);

        let download_value = format!(
            "[osu!direct]({url}/osudirect/{mapset_id})\n\
            [Mapset]({OSU_BASE}d/{mapset_id})\n\
            [No Video]({OSU_BASE}d/{mapset_id}n)\n\
            [Beatconnect](https://beatconnect.io/b/{mapset_id})",
            url = CONFIG.get().unwrap().server.external_url,
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

        let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
        let mut rosu_map = Map::from_path(map_path).await.map_err(PpError::from)?;
        let mod_bits = mods.bits();

        let mod_mult = 0.5_f32.powi(
            mods.contains(GameMods::Easy) as i32
                + mods.contains(GameMods::NoFail) as i32
                + mods.contains(GameMods::HalfTime) as i32,
        );

        if let Some(ar_) = attrs.ar {
            rosu_map.ar = ar_ as f32;
        }

        if let Some(cs_) = attrs.cs {
            rosu_map.cs = cs_ as f32;
        }

        if let Some(hp_) = attrs.hp {
            rosu_map.hp = hp_ as f32;
        }

        if let Some(od_) = attrs.od {
            rosu_map.od = od_ as f32;
        }

        let attributes = rosu_map.attributes().mods(mod_bits);
        let hp = attributes.hp;
        let cs = attributes.cs;

        let (ar, od) = if map.mode == GameMode::MNA {
            (rosu_map.ar as f64, rosu_map.od)
        } else {
            let mult = mod_bits.od_ar_hp_multiplier() as f32;

            (
                calculate_ar((rosu_map.ar * mult).min(10.0), attributes.clock_rate as f32) as f64,
                calculate_od((rosu_map.od * mult).min(10.0), attributes.clock_rate as f32),
            )
        };

        let mut attributes = rosu_map.stars().mods(mod_bits).calculate();
        let stars = attributes.stars();
        let mut pps = Vec::with_capacity(4);

        for acc in [95.0, 97.0, 99.0, 100.0].iter().copied() {
            let pp_result: PerformanceAttributes = match rosu_map.mode {
                Mode::STD => OsuPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(attributes)
                    .accuracy(acc)
                    .calculate()
                    .into(),
                Mode::MNA => ManiaPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(attributes)
                    .score(acc_to_score(mod_mult, acc as f32) as u32)
                    .calculate()
                    .into(),
                Mode::CTB => CatchPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(attributes)
                    .accuracy(acc)
                    .calculate()
                    .into(),
                Mode::TKO => TaikoPP::new(&rosu_map)
                    .mods(mod_bits)
                    .attributes(attributes)
                    .accuracy(acc)
                    .calculate()
                    .into(),
            };

            pps.push(pp_result.pp() as f32);
            attributes = pp_result.into();
        }

        let mut pp_values = String::with_capacity(128);

        let len = if rosu_map.mode == Mode::MNA {
            let len = 9.max(2 + format!("{:.2}", pps[3]).len());
            pp_values.push_str("```\n");

            let _ = writeln!(
                pp_values,
                "    |{:^len$}|{:^len$}|{:^len$}|{:^len$}",
                with_comma_int(acc_to_score(mod_mult, 95.0)).to_string(),
                with_comma_int(acc_to_score(mod_mult, 97.0)).to_string(),
                with_comma_int(acc_to_score(mod_mult, 99.0)).to_string(),
                with_comma_int(acc_to_score(mod_mult, 100.0)).to_string(),
            );

            len
        } else {
            let len = 6.max(2 + format!("{:.2}", pps[3]).len());
            pp_values.push_str("```\n");

            let _ = writeln!(
                pp_values,
                "Acc |{:^len$}|{:^len$}|{:^len$}|{:^len$}",
                "95%", "97%", "99%", "100%",
            );

            len
        };

        let _ = writeln!(
            pp_values,
            "----+{:->len$}+{:->len$}+{:->len$}+{:->len$}",
            "-", "-", "-", "-",
        );

        let _ = writeln!(
            pp_values,
            " PP |{:^len$}|{:^len$}|{:^len$}|{:^len$}",
            round(pps[0]),
            round(pps[1]),
            round(pps[2]),
            round(pps[3]),
        );

        pp_values.push_str("```");

        if let Some(combo) = map.max_combo {
            let _ = write!(info_value, "Combo: `{combo}x`");
        }

        let _ = writeln!(info_value, " Stars: `{stars:.2}★`");
        let _ = write!(info_value, "Length: `{}` ", sec_to_minsec(seconds_total));

        if seconds_drain != seconds_total {
            let _ = write!(info_value, "(`{}`) ", sec_to_minsec(seconds_drain));
        }

        let _ = write!(
            info_value,
            "BPM: `{}` Objects: `{}`\nCS: `{}` AR: `{}` OD: `{}` HP: `{}` Spinners: `{}`",
            round(bpm),
            map.count_objects(),
            round(cs as f32),
            round(ar as f32),
            round(od),
            round(hp as f32),
            map.count_spinners,
        );

        let mut info_name = format!("{} __[{}]__", mode_emote(map.mode), map.version);

        if !mods.is_empty() {
            let _ = write!(info_name, " +{mods}");
        }

        fields.push(field!(info_name, info_value, true));
        fields.push(field!("Download", download_value, true));

        let mut field_name = format!(
            ":heart: {}  :play_pause: {}  | {:?}, {:?}",
            with_comma_int(mapset.favourite_count),
            with_comma_int(mapset.playcount),
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

        let creator_avatar_url = mapset.creator.as_ref().map_or_else(
            || format!("{}{}", AVATAR_URL, mapset.creator_id),
            |creator| creator.avatar_url.to_owned(),
        );

        let author = AuthorBuilder::new(format!("Created by {}", mapset.creator_name))
            .url(format!("{OSU_BASE}u/{}", mapset.creator_id))
            .icon_url(creator_avatar_url);

        let footer_text = format!(
            "Map {} out of {} in the mapset, {date_text}",
            pages.0, pages.1
        );

        let footer = FooterBuilder::new(footer_text);

        let thumbnail = with_thumbnail
            .then(|| mapset.covers.cover.to_owned())
            .unwrap_or_default();

        let image = (!with_thumbnail)
            .then(|| attachment("map_graph.png"))
            .unwrap_or_default();

        let mut description = format!(
            ":musical_note: [Song preview](https://b.ppy.sh/preview/{mapset_id}.mp3) \
            :frame_photo: [Full background](https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg)",
            mapset_id = mapset.mapset_id
        );

        if map.mode == GameMode::STD {
            let _ = write!(
                description,
                " :clapper: [Map preview](http://jmir.xyz/osu/preview.html#{map_id})",
                map_id = map.map_id
            );
        }

        Ok(Self {
            title,
            image,
            footer,
            fields,
            author,
            thumbnail,
            timestamp,
            description,
            url: map.url.to_owned(),
        })
    }
}

fn acc_to_score(mod_mult: f32, acc: f32) -> u64 {
    (mod_mult * (acc * 10_000.0 - (100.0 - acc) * 50_000.0)).round() as u64
}
