use crate::{
    embeds::{osu, Author, EmbedBuilder, EmbedData, Footer},
    util::{
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago,
        error::PPError,
        matcher::highlight_funny_numeral,
        numbers::{round, with_comma_float, with_comma_uint},
        osu::{grade_completion_mods, prepare_beatmap_file},
        ScoreExt,
    },
    BotResult,
};

use chrono::{DateTime, Utc};
use rosu_pp::{Beatmap as Map, BeatmapExt, FruitsPP, ManiaPP, OsuPP, TaikoPP};
use rosu_v2::prelude::{Beatmap, BeatmapUserScore, GameMode, GameMods, Grade, Score, User};
use std::{borrow::Cow, fmt::Write};
use tokio::fs::File;

const GLOBAL_IDX_THRESHOLD: usize = 500;

pub struct CompareEmbed {
    author: Author,
    description: String,
    footer: Footer,
    thumbnail: String,
    timestamp: DateTime<Utc>,
    title: String,
    url: String,

    mapset_id: u32,
    stars: f32,
    grade_completion_mods: Cow<'static, str>,
    score: String,
    acc: f32,
    ago: String,
    pp: String,
    combo: String,
    hits: String,
    map_info: String,
}

impl CompareEmbed {
    pub async fn new(
        user: &User,
        personal: Option<&[Score]>,
        map_score: BeatmapUserScore,
        with_mods: bool,
    ) -> BotResult<Self> {
        let score = &map_score.score;
        let map = score.map.as_ref().unwrap();
        let mapset = score.mapset.as_ref().unwrap();

        let map_path = prepare_beatmap_file(map.map_id).await?;
        let file = File::open(map_path).await.map_err(PPError::from)?;
        let rosu_map = Map::parse(file).await.map_err(PPError::from)?;
        let mods = score.mods.bits();
        let max_result = rosu_map.max_pp(mods);
        let attributes = max_result.attributes;

        let max_pp = max_result.pp;
        let stars = round(attributes.stars());

        let pp = if score.grade == Grade::F {
            let hits = score.total_hits() as usize;

            let pp_result = match map.mode {
                GameMode::STD => OsuPP::new(&rosu_map)
                    .mods(mods)
                    .combo(score.max_combo as usize)
                    .n300(score.statistics.count_300 as usize)
                    .n100(score.statistics.count_100 as usize)
                    .n50(score.statistics.count_50 as usize)
                    .misses(score.statistics.count_miss as usize)
                    .passed_objects(hits)
                    .calculate(),
                GameMode::MNA => ManiaPP::new(&rosu_map)
                    .mods(mods)
                    .score(score.score)
                    .passed_objects(hits)
                    .calculate(),
                GameMode::CTB => FruitsPP::new(&rosu_map)
                    .mods(mods)
                    .combo(score.max_combo as usize)
                    .fruits(score.statistics.count_300 as usize)
                    .droplets(score.statistics.count_100 as usize)
                    .misses(score.statistics.count_miss as usize)
                    .passed_objects(hits - score.statistics.count_katu as usize)
                    .accuracy(score.accuracy)
                    .calculate(),
                GameMode::TKO => TaikoPP::new(&rosu_map)
                    .combo(score.max_combo as usize)
                    .mods(mods)
                    .passed_objects(hits)
                    .accuracy(score.accuracy)
                    .calculate(),
            };

            pp_result.pp
        } else if let Some(pp) = score.pp {
            pp
        } else {
            let pp_result = match map.mode {
                GameMode::STD => OsuPP::new(&rosu_map)
                    .attributes(attributes)
                    .mods(mods)
                    .combo(score.max_combo as usize)
                    .n300(score.statistics.count_300 as usize)
                    .n100(score.statistics.count_100 as usize)
                    .n50(score.statistics.count_50 as usize)
                    .misses(score.statistics.count_miss as usize)
                    .calculate(),
                GameMode::MNA => ManiaPP::new(&rosu_map)
                    .attributes(attributes)
                    .mods(mods)
                    .score(score.score)
                    .calculate(),
                GameMode::CTB => FruitsPP::new(&rosu_map)
                    .attributes(attributes)
                    .mods(mods)
                    .combo(score.max_combo as usize)
                    .fruits(score.statistics.count_300 as usize)
                    .droplets(score.statistics.count_100 as usize)
                    .misses(score.statistics.count_miss as usize)
                    .accuracy(score.accuracy)
                    .calculate(),
                GameMode::TKO => TaikoPP::new(&rosu_map)
                    .attributes(attributes)
                    .combo(score.max_combo as usize)
                    .mods(mods)
                    .misses(score.statistics.count_miss as usize)
                    .accuracy(score.accuracy)
                    .calculate(),
            };

            pp_result.pp
        };

        let pp = osu::get_pp(Some(pp), Some(max_pp));
        let hits = score.hits_string(map.mode);
        let grade_completion_mods = grade_completion_mods(score, map);

        let (combo, title) = if map.mode == GameMode::MNA {
            let mut ratio = score.statistics.count_geki as f32;

            if score.statistics.count_300 > 0 {
                ratio /= score.statistics.count_300 as f32
            }

            let combo = format!("**{}x** / {:.2}", &score.max_combo, ratio);

            let title = format!(
                "{} {} - {} [{}]",
                osu::get_keys(score.mods, &map),
                mapset.artist,
                mapset.title,
                map.version
            );

            (combo, title)
        } else {
            (
                osu::get_combo(score, map),
                format!("{} - {} [{}]", mapset.artist, mapset.title, map.version),
            )
        };

        let footer = Footer::new(format!(
            "{:?} map by {} | played",
            map.status, mapset.creator_name
        ))
        .icon_url(format!("{}{}", AVATAR_URL, mapset.creator_id));

        let personal_idx = personal.and_then(|personal| personal.iter().position(|s| s == score));
        let global_idx = map_score.pos;

        let description = if personal_idx.is_some() || global_idx <= GLOBAL_IDX_THRESHOLD {
            let mut description = String::with_capacity(25);

            if personal_idx.is_some() || global_idx <= 50 {
                description.push_str("__**");
            }

            if let Some(idx) = personal_idx {
                let _ = write!(description, "Personal Best #{}", idx + 1);

                if global_idx <= GLOBAL_IDX_THRESHOLD {
                    description.reserve(19 + 18 * with_mods as usize);
                    description.push_str(" and ");
                }
            }

            if global_idx <= GLOBAL_IDX_THRESHOLD {
                let _ = write!(description, "Global Top #{}", global_idx);
            }

            if personal_idx.is_some() || global_idx <= 50 {
                description.push_str("**__");
            }

            if with_mods && global_idx <= GLOBAL_IDX_THRESHOLD {
                description.push_str(" (Mod leaderboard)");
            }

            description
        } else {
            String::new()
        };

        let acc = round(score.accuracy);
        let ago = how_long_ago(&score.created_at).to_string();
        let timestamp = score.created_at;
        let mods = score.mods;
        let score = with_comma_uint(score.score).to_string();

        Ok(Self {
            description,
            title,
            url: map.url.to_owned(),
            author: author!(user),
            footer,
            timestamp,
            thumbnail: format!("{}{}l.jpg", MAP_THUMB_URL, map.mapset_id), // mapset.covers is empty :(
            grade_completion_mods,
            stars,
            score,
            acc,
            ago,
            pp,
            combo,
            hits,
            mapset_id: mapset.mapset_id,
            map_info: osu::get_map_info(&map, mods, stars),
        })
    }
}

impl EmbedData for CompareEmbed {
    fn as_builder(&self) -> EmbedBuilder {
        let score = highlight_funny_numeral(&self.score).into_owned();
        let acc = highlight_funny_numeral(&format!("{}%", self.acc)).into_owned();
        let pp = highlight_funny_numeral(&self.pp).into_owned();

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

        fields.reserve(3);

        let mania = self.hits.chars().filter(|&c| c == '/').count() == 5;

        let combo = highlight_funny_numeral(&self.combo).into_owned();
        let hits = highlight_funny_numeral(&self.hits).into_owned();

        fields.push(field!(
            if mania { "Combo / Ratio" } else { "Combo" },
            combo,
            true
        ));

        fields.push(field!("Hits", hits, true));
        fields.push(field!("Map Info", self.map_info.clone(), false));

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
    }

    fn into_builder(self) -> EmbedBuilder {
        let name = format!(
            "{}\t{}\t({}%)\t{}",
            self.grade_completion_mods, self.score, self.acc, self.ago
        );

        let value = format!("{} [ {} ] {}", self.pp, self.combo, self.hits);

        let mut title = self.title;
        let _ = write!(title, " [{}★]", self.stars);

        EmbedBuilder::new()
            .author(self.author)
            .description(self.description)
            .fields(vec![field![name, value, false]])
            .thumbnail(self.thumbnail)
            .title(title)
            .url(self.url)
    }
}

pub struct NoScoresEmbed {
    description: String,
    thumbnail: String,
    footer: Footer,
    author: Author,
    title: String,
    url: String,
}

impl NoScoresEmbed {
    pub fn new(user: User, map: Beatmap, mods: Option<GameMods>) -> Self {
        let stats = user.statistics.as_ref().unwrap();
        let mapset = map.mapset.as_ref().unwrap();

        let footer = Footer::new(format!("{:?} map by {}", map.status, mapset.creator_name))
            .icon_url(format!("{}{}", AVATAR_URL, mapset.creator_id));

        let author_text = format!(
            "{name}: {pp}pp (#{global} {country}{national})",
            name = user.username,
            pp = with_comma_float(stats.pp),
            global = with_comma_uint(stats.global_rank.unwrap_or(0)),
            country = user.country_code,
            national = stats.country_rank.unwrap_or(0),
        );

        let author = Author::new(author_text)
            .url(format!("{}u/{}", OSU_BASE, user.user_id))
            .icon_url(user.avatar_url);

        let title = format!("{} - {} [{}]", mapset.artist, mapset.title, map.version);

        let mut description = "No scores".to_owned();

        if let Some(mods) = mods {
            let _ = write!(description, " with {}", mods);
        }

        Self {
            author,
            description,
            footer,
            thumbnail: format!("{}{}l.jpg", MAP_THUMB_URL, map.mapset_id),
            title,
            url: map.url,
        }
    }
}

impl_builder!(NoScoresEmbed {
    author,
    description,
    footer,
    thumbnail,
    title,
    url,
});
