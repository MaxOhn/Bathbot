use std::fmt::Write;

use command_macros::EmbedData;
use eyre::{Result, WrapErr};
use rosu_pp::{AnyPP, Beatmap as Map, BeatmapExt};
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMode, GameMods};
use time::OffsetDateTime;
use twilight_model::channel::embed::EmbedField;

use crate::{
    commands::osu::CustomAttrs,
    core::Context,
    embeds::attachment,
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::{AVATAR_URL, OSU_BASE},
        datetime::sec_to_minsec,
        numbers::{round, with_comma_int},
        osu::{mode_emote, prepare_beatmap_file},
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct MapEmbed {
    title: String,
    url: String,
    description: String,
    footer: FooterBuilder,
    author: AuthorBuilder,
    image: String,
    timestamp: OffsetDateTime,
    fields: Vec<EmbedField>,
}

impl MapEmbed {
    pub async fn new(
        map: &Beatmap,
        mapset: &Beatmapset,
        mods: GameMods,
        attrs: &CustomAttrs,
        ctx: &Context,
        pages: &Pages,
    ) -> Result<Self> {
        let mut title = String::with_capacity(32);

        if map.mode == GameMode::Mania {
            let _ = write!(title, "[{}K] ", map.cs as u32);
        }

        let _ = write!(
            title,
            "{} - {}",
            mapset.artist.cow_escape_markdown(),
            mapset.title.cow_escape_markdown()
        );

        #[cfg(feature = "server")]
        let url = &crate::core::BotConfig::get().server.external_url;
        #[cfg(not(feature = "server"))]
        let url = "";

        let download_value = format!(
            "[osu!direct]({url}/osudirect/{mapset_id})\n\
            [Mapset]({OSU_BASE}d/{mapset_id})\n\
            [No Video]({OSU_BASE}d/{mapset_id}n)\n\
            [Beatconnect](https://beatconnect.io/b/{mapset_id})",
            mapset_id = map.mapset_id,
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

        let map_path = prepare_beatmap_file(ctx, map.map_id)
            .await
            .wrap_err("failed to prepare map")?;

        let mut rosu_map = Map::from_path(map_path)
            .await
            .wrap_err("failed to parse map")?;

        let mod_bits = mods.bits();

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

        let map_attributes = rosu_map.attributes().mods(mod_bits).build();

        let mut attributes = rosu_map.stars().mods(mod_bits).calculate();
        let stars = attributes.stars();
        const ACCS: [f32; 4] = [95.0, 97.0, 99.0, 100.0];
        let mut pps = Vec::with_capacity(ACCS.len());

        for &acc in ACCS.iter() {
            let pp_result = AnyPP::new(&rosu_map)
                .mods(mod_bits)
                .attributes(attributes)
                .accuracy(acc as f64)
                .calculate();

            let pp = pp_result.pp();

            let pp_str = if pp > 100_000.0 {
                format!("{pp:.3e}")
            } else {
                round(pp as f32).to_string()
            };

            pps.push(pp_str);
            attributes = pp_result.into();
        }

        let mut pp_values = String::with_capacity(128);
        let mut lens = Vec::with_capacity(ACCS.len());

        pp_values.push_str("```\nAcc ");

        for (pp, &acc) in pps.iter().zip(&ACCS) {
            let acc = acc.to_string() + "%";
            let len = pp.len().max(acc.len()) + 2;
            let _ = write!(pp_values, "|{acc:^len$}");
            lens.push(len);
        }

        pp_values.push_str("\n----");

        for len in lens.iter() {
            let _ = write!(pp_values, "+{:->len$}", "-");
        }

        pp_values.push_str("\n PP ");

        for (pp, len) in pps.iter().zip(&lens) {
            let _ = write!(pp_values, "|{pp:^len$}");
        }

        pp_values.push_str("\n```");

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
            round(map_attributes.cs as f32),
            round(map_attributes.ar as f32),
            round(map_attributes.od as f32),
            round(map_attributes.hp as f32),
            map.count_spinners,
        );

        let mut info_name = format!(
            "{mode} __[{version}]__",
            mode = mode_emote(map.mode),
            version = map.version.cow_escape_markdown()
        );

        if !mods.is_empty() {
            let _ = write!(info_name, " +{mods}");
        }

        fields![fields {
            info_name, info_value, true;
            "Download", download_value, true;
        }];

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

        fields![fields { field_name, pp_values, false }];

        let (date_text, timestamp) = if let Some(ranked_date) = mapset.ranked_date {
            (format!("{:?}", map.status), ranked_date)
        } else {
            ("Last updated".to_owned(), map.last_updated)
        };

        let creator_avatar_url = mapset.creator.as_ref().map_or_else(
            || format!("{AVATAR_URL}{}", mapset.creator_id),
            |creator| creator.avatar_url.to_owned(),
        );

        let author = AuthorBuilder::new(format!("Created by {}", mapset.creator_name))
            .url(format!("{OSU_BASE}u/{}", mapset.creator_id))
            .icon_url(creator_avatar_url);

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer_text = format!("Map {page} out of {pages} in the mapset, {date_text}");

        let footer = FooterBuilder::new(footer_text);

        let image = attachment("map_graph.png");

        let mut description = format!(
            ":musical_note: [Song preview](https://b.ppy.sh/preview/{mapset_id}.mp3) \
            :frame_photo: [Full background](https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg)",
            mapset_id = mapset.mapset_id
        );

        if map.mode == GameMode::Osu {
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
            timestamp,
            description,
            url: map.url.to_owned(),
        })
    }
}
