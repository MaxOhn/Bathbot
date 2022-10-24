use std::{borrow::Cow, fmt::Write};

use command_macros::EmbedData;
use eyre::{Result, WrapErr};
use rosu_pp::{Beatmap as Map, BeatmapExt};
use rosu_v2::prelude::{Beatmap, GameMode, Grade, Score, User};
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

use crate::{
    core::Context,
    database::MinimizedPp,
    embeds::osu,
    util::{
        builder::{AuthorBuilder, EmbedBuilder, FooterBuilder},
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::{how_long_ago_dynamic, HowLongAgoFormatterDynamic},
        matcher::highlight_funny_numeral,
        numbers::{self, round, with_comma_float, with_comma_int},
        osu::{flag_url, grade_completion_mods, prepare_beatmap_file, ModSelection},
        CowUtils, ScoreExt,
    },
};

use super::IfFC;

const GLOBAL_IDX_THRESHOLD: usize = 500;

pub struct CompareEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
    timestamp: OffsetDateTime,
    title: String,
    url: String,

    mapset_id: u32,
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
    minimized_pp: MinimizedPp,
}

impl CompareEmbed {
    pub async fn new(
        personal: Option<&[Score]>,
        score: &Score,
        global_idx: usize,
        pinned: bool,
        minimized_pp: MinimizedPp,
        ctx: &Context,
    ) -> Result<Self> {
        let user = score.user.as_ref().unwrap();
        let map = score.map.as_ref().unwrap();
        let mapset = score.mapset.as_ref().unwrap();

        let map_path = prepare_beatmap_file(ctx, map.map_id)
            .await
            .wrap_err("failed to prepare map")?;

        let rosu_map = Map::from_path(map_path)
            .await
            .wrap_err("failed to parse map")?;

        let mods = score.mods.bits();
        let attrs = rosu_map.max_pp(mods);

        let max_pp = attrs.pp();
        let stars = round(attrs.stars() as f32);

        let (if_fc, attrs) = IfFC::new(score, &rosu_map, attrs.difficulty_attributes(), mods);

        let if_fc = if_fc.map(|if_fc| {
            let mut hits = String::from("{");
            let _ = write!(hits, "{}/{}/", if_fc.n300, if_fc.n100);

            if let Some(n50) = if_fc.n50 {
                let _ = write!(hits, "{n50}/");
            }

            let _ = write!(hits, "0}}");

            (if_fc.pp, round(if_fc.acc), hits)
        });

        let pp = if score.grade == Grade::F {
            rosu_map
                .pp()
                .mods(mods)
                .combo(score.max_combo as usize)
                .n_geki(score.statistics.count_geki as usize)
                .n300(score.statistics.count_300 as usize)
                .n_katu(score.statistics.count_katu as usize)
                .n100(score.statistics.count_100 as usize)
                .n50(score.statistics.count_50 as usize)
                .n_misses(score.statistics.count_miss as usize)
                .passed_objects(score.total_hits() as usize)
                .calculate()
                .pp() as f32
        } else if let Some(pp) = score.pp {
            pp
        } else {
            rosu_map
                .pp()
                .attributes(attrs)
                .mods(mods)
                .combo(score.max_combo as usize)
                .n_geki(score.statistics.count_geki as usize)
                .n300(score.statistics.count_300 as usize)
                .n_katu(score.statistics.count_katu as usize)
                .n100(score.statistics.count_100 as usize)
                .n50(score.statistics.count_50 as usize)
                .n_misses(score.statistics.count_miss as usize)
                .calculate()
                .pp() as f32
        };

        let pp = Some(pp);
        let max_pp = Some(max_pp as f32);
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
            (
                osu::get_combo(score, map),
                format!(
                    "{} - {} [{}]",
                    mapset.artist.cow_escape_markdown(),
                    mapset.title.cow_escape_markdown(),
                    map.version.cow_escape_markdown()
                ),
            )
        };

        let footer = FooterBuilder::new(format!(
            "{:?} map by {} | played",
            map.status, mapset.creator_name
        ))
        .icon_url(format!("{AVATAR_URL}{}", mapset.creator_id));

        let personal_idx = personal.and_then(|personal| personal.iter().position(|s| s == score));

        let mut description = String::new();

        if pinned {
            description.push('📌');

            if personal_idx.is_some() || global_idx <= GLOBAL_IDX_THRESHOLD {
                description.push(' ');
            } else {
                description.push('\u{200b}'); // zero-width character
            }
        }

        if personal_idx.is_some() || global_idx <= GLOBAL_IDX_THRESHOLD {
            if personal_idx.is_some() || global_idx <= 50 {
                description.push_str("__**");
            }

            if let Some(idx) = personal_idx {
                let _ = write!(description, "Personal Best #{}", idx + 1);

                if global_idx <= GLOBAL_IDX_THRESHOLD {
                    description.reserve(19);
                    description.push_str(" and ");
                }
            }

            if global_idx <= GLOBAL_IDX_THRESHOLD {
                let _ = write!(description, "Global Top #{global_idx}");
            }

            if personal_idx.is_some() || global_idx <= 50 {
                description.push_str("**__");
            }
        }

        let author = {
            let stats = user.statistics.as_ref().expect("no statistics on user");

            let text = format!(
                "{name}: {pp}pp (#{global} {country}{national})",
                name = user.username,
                pp = numbers::with_comma_float(stats.pp),
                global = numbers::with_comma_int(stats.global_rank.unwrap_or(0)),
                country = user.country_code,
                national = stats.country_rank.unwrap_or(0)
            );

            let url = format!("{OSU_BASE}users/{}/{}", user.user_id, score.mode);
            let icon = flag_url(user.country_code.as_str());

            AuthorBuilder::new(text).url(url).icon_url(icon)
        };

        let acc = round(score.accuracy);
        let ago = how_long_ago_dynamic(&score.ended_at);
        let timestamp = score.ended_at;
        let mods = score.mods;
        let score = with_comma_int(score.score).to_string();

        Ok(Self {
            description,
            title,
            url: map.url.to_owned(),
            author,
            footer,
            timestamp,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id), // mapset.covers is empty :(
            grade_completion_mods,
            stars,
            score,
            acc,
            ago,
            pp,
            max_pp,
            combo,
            hits,
            mapset_id: mapset.mapset_id,
            if_fc,
            map_info: osu::get_map_info(map, mods, stars),
            minimized_pp,
        })
    }

    pub fn into_minimized(self) -> Embed {
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

        EmbedBuilder::new()
            .author(self.author)
            .description(self.description)
            .fields(fields![name, value, false])
            .thumbnail(self.thumbnail)
            .title(title)
            .url(self.url)
            .build()
    }

    pub fn as_maximized(&self) -> Embed {
        let score = highlight_funny_numeral(&self.score).into_owned();
        let acc = highlight_funny_numeral(&format!("{}%", self.acc)).into_owned();

        let pp = osu::get_pp(self.pp, self.max_pp);
        let pp = highlight_funny_numeral(&pp).into_owned();

        let mut fields = fields![
            "Grade", self.grade_completion_mods.as_ref().to_owned(), true;
            "Score", score, true;
            "Acc", acc, true;
            "PP", pp, true;
        ];

        fields.reserve(3);

        let mania = self.hits.chars().filter(|&c| c == '/').count() == 5;

        let combo = highlight_funny_numeral(&self.combo).into_owned();
        let hits = highlight_funny_numeral(&self.hits).into_owned();

        fields![fields {
            if mania { "Combo / Ratio" } else { "Combo" }, combo, true;
            "Hits", hits, true;
        }];

        if let Some((pp, acc, hits)) = &self.if_fc {
            let pp = osu::get_pp(Some(*pp), self.max_pp);

            fields![fields {
                "**If FC**: PP", pp, true;
                "Acc", format!("{acc}%"), true;
                "Hits", hits.clone(), true;
            }];
        }

        fields![fields { "Map Info", self.map_info.clone(), false }];

        let image = format!(
            "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
            self.mapset_id
        );

        EmbedBuilder::new()
            .author(&self.author)
            .description(&self.description)
            .fields(fields)
            .footer(&self.footer)
            .image(image)
            .timestamp(self.timestamp)
            .title(&self.title)
            .url(&self.url)
            .build()
    }
}

#[derive(EmbedData)]
pub struct NoScoresEmbed {
    description: &'static str,
    thumbnail: String,
    footer: FooterBuilder,
    author: AuthorBuilder,
    title: String,
    url: String,
}

impl NoScoresEmbed {
    pub fn new(user: User, map: Beatmap, mods: Option<ModSelection>) -> Self {
        let stats = user.statistics.as_ref().unwrap();
        let mapset = map.mapset.as_ref().unwrap();

        let footer = FooterBuilder::new(format!("{:?} map by {}", map.status, mapset.creator_name))
            .icon_url(format!("{AVATAR_URL}{}", mapset.creator_id));

        let author_text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = user.username,
            pp = with_comma_float(stats.pp),
            global = with_comma_int(stats.global_rank.unwrap_or(0)),
            country = user.country_code,
            national = stats.country_rank.unwrap_or(0),
        );

        let author = AuthorBuilder::new(author_text)
            .url(format!("{OSU_BASE}u/{}", user.user_id))
            .icon_url(user.avatar_url);

        let title = format!(
            "{} - {} [{}]",
            mapset.artist.cow_escape_markdown(),
            mapset.title.cow_escape_markdown(),
            map.version.cow_escape_markdown()
        );

        let description = if mods.is_some() {
            "No scores with these mods"
        } else {
            "No scores"
        };

        Self {
            author,
            description,
            footer,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id),
            title,
            url: map.url,
        }
    }
}
