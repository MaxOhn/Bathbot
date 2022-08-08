use crate::{
    core::Context,
    custom_client::TwitchVideo,
    database::MinimizedPp,
    embeds::osu,
    error::PpError,
    util::{
        builder::{AuthorBuilder, EmbedBuilder, FooterBuilder},
        constants::{AVATAR_URL, TWITCH_BASE},
        datetime::{how_long_ago_dynamic, HowLongAgoFormatterDynamic},
        matcher::highlight_funny_numeral,
        numbers::{round, with_comma_int},
        osu::{grade_completion_mods, prepare_beatmap_file},
        CowUtils, Emote, ScoreExt,
    },
    BotResult,
};

use rosu_pp::{
    Beatmap as Map, BeatmapExt, CatchPP, DifficultyAttributes, ManiaPP, OsuPP,
    PerformanceAttributes, TaikoPP,
};
use rosu_v2::prelude::{BeatmapUserScore, GameMode, Grade, RankStatus, Score, User};
use std::{borrow::Cow, cmp::Ordering, fmt::Write};
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

pub struct RecentEmbed {
    description: String,
    title: String,
    url: String,
    author: AuthorBuilder,
    footer: FooterBuilder,
    timestamp: OffsetDateTime,
    thumbnail: String,

    stars: f32,
    grade_completion_mods: Cow<'static, str>,
    score: String,
    acc: f32,
    ago: HowLongAgoFormatterDynamic,
    pp: Option<f32>,
    max_pp: Option<f32>,
    combo: String,
    hits: String,
    if_fc: Option<(f32, f32, String)>,
    map_info: String,
    mapset_cover: String,
    twitch_vod: Option<TwitchVideo>,
    minimized_pp: MinimizedPp,
}

impl RecentEmbed {
    pub async fn new(
        user: &User,
        score: &Score,
        personal: Option<&[Score]>,
        map_score: Option<&BeatmapUserScore>,
        twitch_vod: Option<TwitchVideo>,
        minimized_pp: MinimizedPp,
        ctx: &Context,
    ) -> BotResult<Self> {
        let map = score.map.as_ref().unwrap();
        let mapset = score.mapset.as_ref().unwrap();

        let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
        let rosu_map = Map::from_path(map_path).await.map_err(PpError::from)?;
        let mods = score.mods.bits();
        let max_result = rosu_map.max_pp(mods);
        let mut attributes = max_result.difficulty_attributes();

        let max_pp = score
            .pp
            .filter(|pp| {
                score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania && *pp > 0.0
            })
            .unwrap_or(max_result.pp() as f32);

        let stars = round(attributes.stars() as f32);

        let pp = if let Some(pp) = score.pp {
            pp
        } else if score.grade == Grade::F {
            let hits = score.total_hits() as usize;

            // TODO: simplify
            match map.mode {
                GameMode::Osu => {
                    OsuPP::new(&rosu_map)
                        .mods(mods)
                        .combo(score.max_combo as usize)
                        .n300(score.statistics.count_300 as usize)
                        .n100(score.statistics.count_100 as usize)
                        .n50(score.statistics.count_50 as usize)
                        .misses(score.statistics.count_miss as usize)
                        .passed_objects(hits)
                        .calculate()
                        .pp as f32
                }
                GameMode::Mania => {
                    ManiaPP::new(&rosu_map)
                        .mods(mods)
                        .score(score.score)
                        .passed_objects(hits)
                        .calculate()
                        .pp as f32
                }
                GameMode::Catch => {
                    CatchPP::new(&rosu_map)
                        .mods(mods)
                        .combo(score.max_combo as usize)
                        .fruits(score.statistics.count_300 as usize)
                        .droplets(score.statistics.count_100 as usize)
                        .misses(score.statistics.count_miss as usize)
                        .passed_objects(hits - score.statistics.count_katu as usize)
                        .accuracy(score.accuracy as f64)
                        .calculate()
                        .pp as f32
                }
                GameMode::Taiko => {
                    TaikoPP::new(&rosu_map)
                        .combo(score.max_combo as usize)
                        .mods(mods)
                        .passed_objects(hits)
                        .accuracy(score.accuracy as f64)
                        .calculate()
                        .pp as f32
                }
            }
        } else {
            // TODO: simplify
            let pp_result: PerformanceAttributes = match map.mode {
                GameMode::Osu => OsuPP::new(&rosu_map)
                    .attributes(attributes)
                    .mods(mods)
                    .combo(score.max_combo as usize)
                    .n300(score.statistics.count_300 as usize)
                    .n100(score.statistics.count_100 as usize)
                    .n50(score.statistics.count_50 as usize)
                    .misses(score.statistics.count_miss as usize)
                    .calculate()
                    .into(),
                GameMode::Mania => ManiaPP::new(&rosu_map)
                    .attributes(attributes)
                    .mods(mods)
                    .score(score.score)
                    .calculate()
                    .into(),
                GameMode::Catch => CatchPP::new(&rosu_map)
                    .attributes(attributes)
                    .mods(mods)
                    .combo(score.max_combo as usize)
                    .fruits(score.statistics.count_300 as usize)
                    .droplets(score.statistics.count_100 as usize)
                    .misses(score.statistics.count_miss as usize)
                    .accuracy(score.accuracy as f64)
                    .calculate()
                    .into(),
                GameMode::Taiko => TaikoPP::new(&rosu_map)
                    .attributes(attributes)
                    .combo(score.max_combo as usize)
                    .mods(mods)
                    .misses(score.statistics.count_miss as usize)
                    .accuracy(score.accuracy as f64)
                    .calculate()
                    .into(),
            };

            let pp = pp_result.pp();
            attributes = pp_result.into();

            pp as f32
        };

        let (if_fc, _) = IfFC::new(score, &rosu_map, attributes, mods);

        let pp = Some(pp);
        let max_pp = Some(max_pp);
        let hits = score.hits_string(map.mode);
        let grade_completion_mods = grade_completion_mods(score, map);

        let (combo, title) = if map.mode == GameMode::Mania {
            let mut ratio = score.statistics.count_geki as f32;

            if score.statistics.count_300 > 0 {
                ratio /= score.statistics.count_300 as f32
            }

            let combo = format!("**{}x** / {ratio:.2}", &score.max_combo);

            let title = format!(
                "{} {} - {} [{}]",
                osu::get_keys(score.mods, map),
                mapset.artist.cow_escape_markdown(),
                mapset.title.cow_escape_markdown(),
                map.version.cow_escape_markdown(),
            );

            (combo, title)
        } else {
            let combo = osu::get_combo(score, map);

            let title = format!(
                "{} - {} [{}]",
                mapset.artist.cow_escape_markdown(),
                mapset.title.cow_escape_markdown(),
                map.version.cow_escape_markdown(),
            );

            (combo, title)
        };

        let if_fc = if_fc.map(|if_fc| {
            let mut hits = String::from("{");
            let _ = write!(hits, "{}/{}/", if_fc.n300, if_fc.n100);

            if let Some(n50) = if_fc.n50 {
                let _ = write!(hits, "{n50}/");
            }

            let _ = write!(hits, "0}}");

            (if_fc.pp, round(if_fc.acc), hits)
        });

        let footer = FooterBuilder::new(format!(
            "{:?} map by {} | played",
            map.status, mapset.creator_name
        ))
        .icon_url(format!("{AVATAR_URL}{}", mapset.creator_id));

        let personal_idx = personal
            .filter(|_| matches!(map.status, RankStatus::Ranked))
            .filter(|personal| {
                personal
                    .last()
                    .map_or(true, |last| last.pp < score.pp || personal.len() < 100)
            })
            .and_then(|personal| {
                personal
                    .iter()
                    .position(|s| s == score)
                    .or_else(|| {
                        personal
                            .binary_search_by(|probe| {
                                score.pp.partial_cmp(&probe.pp).unwrap_or(Ordering::Equal)
                            })
                            .map_or_else(Some, Some)
                            .filter(|&idx| idx < 100)
                    })
                    .map(|idx| idx + 1)
            });

        let global_idx = map_score
            .and_then(|s| (&s.score == score).then(|| s.pos))
            .filter(|&p| p <= 50);

        let description = if personal_idx.is_some() || global_idx.is_some() {
            let mut description = String::with_capacity(25);
            description.push_str("__**");

            if let Some(idx) = personal_idx {
                let _ = write!(description, "Personal Best #{idx}");

                if global_idx.is_some() {
                    description.reserve(19);
                    description.push_str(" and ");
                }
            }

            if let Some(idx) = global_idx {
                let _ = write!(description, "Global Top #{idx}");
            }

            description.push_str("**__");

            description
        } else {
            String::new()
        };

        Ok(Self {
            description,
            title,
            url: map.url.to_owned(),
            author: author!(user),
            footer,
            timestamp: score.ended_at,
            thumbnail: mapset.covers.list.to_owned(),
            grade_completion_mods,
            stars,
            score: with_comma_int(score.score).to_string(),
            acc: round(score.accuracy),
            ago: how_long_ago_dynamic(&score.ended_at),
            pp,
            max_pp,
            combo,
            hits,
            map_info: osu::get_map_info(map, score.mods, stars),
            if_fc,
            mapset_cover: mapset.covers.cover.to_owned(),
            twitch_vod,
            minimized_pp,
        })
    }

    pub fn as_maximized(&self) -> Embed {
        let score = highlight_funny_numeral(&self.score).into_owned();
        let acc = highlight_funny_numeral(&format!("{}%", self.acc)).into_owned();

        let pp = osu::get_pp(self.pp, self.max_pp);
        let pp = highlight_funny_numeral(&pp).into_owned();

        let mut fields = vec![
            field!(
                "Grade",
                self.grade_completion_mods.as_ref().to_owned(),
                true
            ),
            field!("Score", score, true),
            field!("Acc", acc, true),
            field!("PP", pp, true),
        ];

        fields.reserve(
            3 + (self.if_fc.is_some() as usize) * 3 + (self.twitch_vod.is_some()) as usize * 2,
        );

        let mania = self.hits.chars().filter(|&c| c == '/').count() == 5;

        let combo = highlight_funny_numeral(&self.combo).into_owned();
        let hits = highlight_funny_numeral(&self.hits).into_owned();

        let name = if mania { "Combo / Ratio" } else { "Combo" };

        fields.push(field!(name, combo, true));
        fields.push(field!("Hits", hits, true));

        if let Some((pp, acc, hits)) = &self.if_fc {
            let pp = osu::get_pp(Some(*pp), self.max_pp);
            fields.push(field!("**If FC**: PP", pp, true));
            fields.push(field!("Acc", format!("{acc}%"), true));
            fields.push(field!("Hits", hits.clone(), true));
        }

        fields.push(field!("Map Info".to_owned(), self.map_info.clone(), false));

        if let Some(ref vod) = self.twitch_vod {
            let twitch_channel = format!(
                "[**{name}**]({base}{name})",
                base = TWITCH_BASE,
                name = vod.username
            );

            fields.push(field!("Live on twitch", twitch_channel, true));

            let vod_hyperlink = format!("[**VOD**]({})", vod.url);
            fields.push(field!("Liveplay of this score", vod_hyperlink, true));
        }

        EmbedBuilder::new()
            .author(&self.author)
            .description(&self.description)
            .fields(fields)
            .footer(&self.footer)
            .image(&self.mapset_cover)
            .timestamp(self.timestamp)
            .title(&self.title)
            .url(&self.url)
            .build()
    }

    pub fn into_minimized(mut self) -> Embed {
        let name = format!(
            "{}\t{}\t({}%)\t{}",
            self.grade_completion_mods, self.score, self.acc, self.ago
        );

        let pp = match self.minimized_pp {
            MinimizedPp::IfFc => {
                let mut result = String::with_capacity(17);
                result.push_str("**");

                if let Some(pp) = self.pp {
                    let _ = write!(result, "{:.2}", pp);
                } else {
                    result.push('-');
                }

                if let Some((if_fc, ..)) = self.if_fc {
                    let _ = write!(result, "pp** ~~({if_fc:.2}pp)~~");
                } else {
                    result.push_str("**/");

                    if let Some(max) = self.max_pp {
                        let pp = self.pp.map(|pp| pp.max(max)).unwrap_or(max);
                        let _ = write!(result, "{:.2}", pp);
                    } else {
                        result.push('-');
                    }

                    result.push_str("PP");
                }

                result
            }
            MinimizedPp::Max => osu::get_pp(self.pp, self.max_pp),
        };

        let value = format!("{pp} [ {} ] {}", self.combo, self.hits);

        let mut title = self.title;
        let _ = write!(title, " [{}★]", self.stars);

        let fields = vec![field!(name, value, false)];

        if let Some(ref vod) = self.twitch_vod {
            let _ = write!(
                self.description,
                " {} [Liveplay on twitch]({})",
                Emote::Twitch.text(),
                vod.url
            );
        }

        EmbedBuilder::new()
            .author(self.author)
            .description(self.description)
            .fields(fields)
            .thumbnail(self.thumbnail)
            .title(title)
            .url(self.url)
            .build()
    }
}

pub struct IfFC {
    pub n300: usize,
    pub n100: usize,
    pub n50: Option<usize>,
    pub pp: f32,
    pub acc: f32,
}

impl IfFC {
    pub fn new(
        score: &Score,
        map: &Map,
        attributes: DifficultyAttributes,
        mods: u32,
    ) -> (Option<Self>, DifficultyAttributes) {
        if score.is_fc(score.mode, attributes.max_combo().unwrap_or(0) as u32) {
            return (None, attributes);
        }

        match attributes {
            DifficultyAttributes::Osu(attributes) => {
                let total_objects = (map.n_circles + map.n_sliders + map.n_spinners) as usize;
                let passed_objects = (score.statistics.count_300
                    + score.statistics.count_100
                    + score.statistics.count_50
                    + score.statistics.count_miss) as usize;

                let mut count300 = score.statistics.count_300 as usize
                    + total_objects.saturating_sub(passed_objects);

                let count_hits = total_objects - score.statistics.count_miss as usize;
                let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

                count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
                let count100 = (score.statistics.count_100 + new100s) as usize;
                let count50 = score.statistics.count_50 as usize;

                let pp_result = OsuPP::new(map)
                    .attributes(attributes)
                    .mods(mods)
                    .n300(count300)
                    .n100(count100)
                    .n50(count50)
                    .calculate();

                let acc = 100.0 * (6 * count300 + 2 * count100 + count50) as f32
                    / (6 * total_objects) as f32;

                let if_fc = Self {
                    n300: count300,
                    n100: count100,
                    n50: Some(count50),
                    pp: pp_result.pp as f32,
                    acc,
                };

                (Some(if_fc), pp_result.difficulty.into())
            }
            DifficultyAttributes::Catch(attributes) => {
                let total_objects = attributes.max_combo();
                let passed_objects = (score.statistics.count_300
                    + score.statistics.count_100
                    + score.statistics.count_miss) as usize;

                let missing = total_objects - passed_objects;
                let missing_fruits = missing.saturating_sub(
                    attributes
                        .n_droplets
                        .saturating_sub(score.statistics.count_100 as usize),
                );

                let missing_droplets = missing - missing_fruits;

                let n_fruits = score.statistics.count_300 as usize + missing_fruits;
                let n_droplets = score.statistics.count_100 as usize + missing_droplets;
                let n_tiny_droplet_misses = score.statistics.count_katu as usize;
                let n_tiny_droplets = attributes
                    .n_tiny_droplets
                    .saturating_sub(n_tiny_droplet_misses);

                let pp_result = CatchPP::new(map)
                    .attributes(attributes)
                    .mods(mods)
                    .fruits(n_fruits)
                    .droplets(n_droplets)
                    .tiny_droplets(n_tiny_droplets)
                    .tiny_droplet_misses(n_tiny_droplet_misses)
                    .calculate();

                let hits = n_fruits + n_droplets + n_tiny_droplets;
                let total = hits + n_tiny_droplet_misses;

                let acc = if total == 0 {
                    0.0
                } else {
                    100.0 * hits as f32 / total as f32
                };

                let if_fc = Self {
                    n300: n_fruits,
                    n100: n_droplets,
                    n50: Some(n_tiny_droplets),
                    pp: pp_result.pp as f32,
                    acc,
                };

                (Some(if_fc), pp_result.difficulty.into())
            }
            DifficultyAttributes::Taiko(attributes) => {
                let total_objects = map.n_circles as usize;
                let passed_objects = score.total_hits() as usize;

                let mut count300 = score.statistics.count_300 as usize
                    + total_objects.saturating_sub(passed_objects);

                let count_hits = total_objects - score.statistics.count_miss as usize;
                let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

                count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
                let count100 = (score.statistics.count_100 + new100s) as usize;

                let acc = 100.0 * (2 * count300 + count100) as f32 / (2 * total_objects) as f32;

                let pp_result = TaikoPP::new(map)
                    .attributes(attributes)
                    .mods(mods)
                    .accuracy(acc as f64)
                    .calculate();

                let if_fc = Self {
                    n300: count300,
                    n100: count100,
                    n50: None,
                    pp: pp_result.pp as f32,
                    acc,
                };

                (Some(if_fc), pp_result.difficulty.into())
            }
            DifficultyAttributes::Mania(_) => (None, attributes),
        }
    }
}
