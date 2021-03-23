use crate::{
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        constants::{AVATAR_URL, DARK_GREEN, MAP_THUMB_URL, OSU_BASE},
        datetime::how_long_ago,
        error::PPError,
        matcher::highlight_funny_numeral,
        numbers::{round, with_comma, with_comma_u64},
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
use twilight_embed_builder::{
    author::EmbedAuthorBuilder, builder::EmbedBuilder, image_source::ImageSource,
};
use twilight_model::channel::embed::EmbedField;

const GLOBAL_IDX_THRESHOLD: usize = 500;

pub struct CompareEmbed {
    description: Option<Cow<'static, str>>,
    title: String,
    url: String,
    author: Author,
    footer: Footer,
    timestamp: DateTime<Utc>,
    thumbnail: ImageSource,
    image: ImageSource,

    stars: f32,
    grade_completion_mods: String,
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

            Some(description.into())
        } else {
            None
        };

        let image = ImageSource::url(format!(
            "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
            map.mapset_id
        ));

        Ok(Self {
            description,
            title,
            url: format!("{}b/{}", OSU_BASE, map.map_id),
            author: author!(user),
            footer,
            timestamp: score.created_at,
            thumbnail: ImageSource::url(format!("{}{}l.jpg", MAP_THUMB_URL, map.mapset_id))
                .unwrap(),
            image: image.unwrap(),
            grade_completion_mods,
            stars,
            score: with_comma_u64(score.score as u64),
            acc: round(score.accuracy),
            ago: how_long_ago(&score.created_at),
            pp,
            combo,
            hits,
            map_info: osu::get_map_info(&map),
        })
    }
}

impl EmbedData for CompareEmbed {
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }

    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }

    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }

    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }

    fn image(&self) -> Option<&ImageSource> {
        Some(&self.image)
    }

    fn timestamp(&self) -> Option<&DateTime<Utc>> {
        Some(&self.timestamp)
    }

    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        let score = highlight_funny_numeral(&self.score).into_owned();
        let acc = highlight_funny_numeral(&format!("{}%", self.acc)).into_owned();
        let pp = highlight_funny_numeral(&self.pp).into_owned();

        let mut fields = vec![
            ("Grade".to_owned(), self.grade_completion_mods.clone(), true),
            ("Score".to_owned(), score, true),
            ("Acc".to_owned(), acc, true),
            ("PP".to_owned(), pp, true),
        ];

        fields.reserve(3);

        let mania = self.hits.chars().filter(|&c| c == '/').count() == 5;

        let combo = highlight_funny_numeral(&self.combo).into_owned();
        let hits = highlight_funny_numeral(&self.hits).into_owned();

        fields.push((
            if mania { "Combo / Ratio" } else { "Combo" }.to_owned(),
            combo,
            true,
        ));

        fields.push(("Hits".to_owned(), hits, true));
        fields.push(("Map Info".to_owned(), self.map_info.clone(), false));

        Some(fields)
    }

    fn minimize(self) -> EmbedBuilder {
        let mut eb = EmbedBuilder::new();

        let name = format!(
            "{}\t{}\t({}%)\t{}",
            self.grade_completion_mods, self.score, self.acc, self.ago
        );

        let value = format!("{} [ {} ] {}", self.pp, self.combo, self.hits);
        let title = format!("{} [{}★]", self.title, self.stars);

        if let Some(description) = self.description {
            eb = eb.description(description).unwrap();
        }

        let ab = EmbedAuthorBuilder::new()
            .name(self.author.name)
            .unwrap()
            .url(self.author.url.unwrap())
            .icon_url(self.author.icon_url.unwrap());

        eb.color(DARK_GREEN)
            .unwrap()
            .thumbnail(self.thumbnail)
            .title(title)
            .unwrap()
            .url(self.url)
            .field(EmbedField {
                name,
                value,
                inline: false,
            })
            .author(ab)
    }
}

pub struct NoScoresEmbed {
    description: Option<String>,
    thumbnail: Option<ImageSource>,
    footer: Option<Footer>,
    author: Option<Author>,
    title: Option<String>,
    url: Option<String>,
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
            pp = with_comma(stats.pp),
            global = with_comma_u64(stats.global_rank.unwrap() as u64),
            country = user.country_code,
            national = stats.country_rank.unwrap_or(0),
        );

        let author = Author::new(author_text)
            .url(format!("{}u/{}", OSU_BASE, user.user_id))
            .icon_url(format!("{}{}", AVATAR_URL, user.user_id));

        let title = format!("{} - {} [{}]", mapset.artist, mapset.title, map.version);

        let mut description = "No scores".to_owned();

        if let Some(mods) = mods {
            let _ = write!(description, " with {}", mods);
        }

        Self {
            description: Some(description),
            footer: Some(footer),
            thumbnail: ImageSource::url(format!("{}{}l.jpg", MAP_THUMB_URL, map.mapset_id)).ok(),
            title: Some(title),
            url: Some(format!("{}b/{}", OSU_BASE, map.map_id)),
            author: Some(author),
        }
    }
}

impl EmbedData for NoScoresEmbed {
    fn description_owned(&mut self) -> Option<String> {
        self.description.take()
    }

    fn url_owned(&mut self) -> Option<String> {
        self.url.take()
    }

    fn title_owned(&mut self) -> Option<String> {
        self.title.take()
    }

    fn footer_owned(&mut self) -> Option<Footer> {
        self.footer.take()
    }

    fn author_owned(&mut self) -> Option<Author> {
        self.author.take()
    }

    fn thumbnail_owned(&mut self) -> Option<ImageSource> {
        self.thumbnail.take()
    }
}
