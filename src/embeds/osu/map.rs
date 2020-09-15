use crate::{
    bail,
    embeds::{Author, EmbedData, Footer},
    pp::roppai::Oppai,
    pp::{Calculations, PPCalculator},
    util::{
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::sec_to_minsec,
        numbers::{round, with_comma_int},
        osu::{mode_emote, prepare_beatmap_file},
    },
    BotResult, Context,
};

use chrono::{DateTime, Utc};
use rosu::models::{Beatmap, GameMode, GameMods};
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct MapEmbed {
    title: String,
    url: String,
    thumbnail: Option<ImageSource>,
    footer: Footer,
    author: Author,
    image: Option<ImageSource>,
    timestamp: DateTime<Utc>,
    fields: Vec<(String, String, bool)>,
}

impl MapEmbed {
    pub async fn new(
        ctx: &Context,
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
        let mut ar = map.diff_ar;
        let mut od = map.diff_od;
        let mut hp = map.diff_hp;
        let mut cs = map.diff_cs;
        let (pp, stars) = match map.mode {
            GameMode::STD | GameMode::TKO => {
                // Prepare oppai
                let map_path = prepare_beatmap_file(map.beatmap_id).await?;
                let mut oppai = Oppai::new();
                if let Err(why) = oppai.set_mods(mods.bits()).calculate(&map_path) {
                    bail!("error while using oppai: {}", why);
                }
                ar = oppai.get_ar();
                od = oppai.get_od();
                hp = oppai.get_hp();
                cs = oppai.get_cs();
                let pp = oppai.get_pp();
                let stars = oppai.get_stars();
                (pp, stars)
            }
            GameMode::MNA | GameMode::CTB => {
                let calculations = Calculations::MAX_PP | Calculations::STARS;
                let mut calculator = PPCalculator::new().map(map);
                if let Err(why) = calculator.calculate(calculations, Some(ctx)).await {
                    warn!("Error while calculating pp for <map: {}", why);
                }
                (
                    calculator.max_pp().unwrap_or_default(),
                    calculator.stars().unwrap_or_default(),
                )
            }
        };
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
        let mut info_value = String::with_capacity(128);
        let _ = write!(info_value, "Max PP: `{:.2}`", pp);
        if let Some(combo) = map.max_combo {
            let _ = write!(info_value, " Combo: `{}x`", combo);
        }
        let _ = writeln!(info_value, " Stars: `{:.2}★`", stars);
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
        let fields = vec![
            (info_name, info_value, true),
            (
                "Download".to_owned(),
                format!(
                    "[Mapset]({base}d/{mapset_id})\n\
                    [No Video]({base}d/{mapset_id}n)\n\
                    [Bloodcat](https://bloodcat.com/osu/s/{mapset_id})\n\
                    <osu://dl/{mapset_id}>",
                    base = OSU_BASE,
                    mapset_id = map.beatmapset_id
                ),
                true,
            ),
            (
                format!(
                    ":heart: {}  :play_pause: {}",
                    with_comma_int(map.favourite_count),
                    with_comma_int(map.playcount)
                ),
                format!("{:?}, {:?}", map.language, map.genre),
                false,
            ),
        ];
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
        Ok(Self {
            title,
            image,
            footer,
            fields,
            author,
            thumbnail,
            timestamp,
            url: format!("{}b/{}", OSU_BASE, map.beatmap_id),
        })
    }
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
