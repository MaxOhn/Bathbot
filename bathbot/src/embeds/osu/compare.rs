use std::{borrow::Cow, fmt::Write};

use bathbot_macros::EmbedData;
use bathbot_psql::model::configs::MinimizedPp;
use bathbot_util::{
    constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
    datetime::HowLongAgoDynamic,
    matcher::highlight_funny_numeral,
    numbers::{round, WithComma},
    osu::ModSelection,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder,
};
use rosu_v2::prelude::{GameMode, Score};
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::CompareEntry,
    manager::{
        redis::{osu::User, RedisData},
        OsuMap,
    },
    util::osu::{grade_completion_mods, IfFc, MapInfo, PersonalBestIndex},
};

use super::{ComboFormatter, HitResultFormatter, KeyFormatter, MessageOrigin, PpFormatter};

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
    mode: GameMode,
    stars: f32,
    grade_completion_mods: Cow<'static, str>,
    score: String,
    acc: f32,
    ago: HowLongAgoDynamic,
    pp: Option<f32>,
    max_pp: Option<f32>,
    combo: String,
    hits: HitResultFormatter,
    if_fc: Option<IfFc>,
    map_info: String,
    minimized_pp: MinimizedPp,
}

impl CompareEmbed {
    pub fn new(
        personal: Option<&[Score]>,
        entry: &CompareEntry,
        user: &RedisData<User>,
        map: &OsuMap,
        global_idx: usize,
        pinned: bool,
        minimized_pp: MinimizedPp,
        origin: &MessageOrigin,
    ) -> Self {
        let CompareEntry {
            score,
            stars,
            max_pp,
            max_combo,
            if_fc,
        } = entry;

        let hits = HitResultFormatter::new(score.mode, score.statistics.clone());
        let grade_completion_mods =
            grade_completion_mods(score.mods, score.grade, score.total_hits(), map);

        let (combo, title) = if map.mode() == GameMode::Mania {
            let mut ratio = score.statistics.count_geki as f32;

            if entry.score.statistics.count_300 > 0 {
                ratio /= score.statistics.count_300 as f32
            }

            let combo = format!("**{}x** / {ratio:.2}", score.max_combo);

            let title = format!(
                "{} {} - {} [{}]",
                KeyFormatter::new(score.mods, map),
                map.artist().cow_escape_markdown(),
                map.title().cow_escape_markdown(),
                map.version().cow_escape_markdown(),
            );

            (combo, title)
        } else {
            let combo = ComboFormatter::new(score.max_combo, Some(*max_combo)).to_string();

            let title = format!(
                "{} - {} [{}]",
                map.artist().cow_escape_markdown(),
                map.title().cow_escape_markdown(),
                map.version().cow_escape_markdown()
            );

            (combo, title)
        };

        let footer = FooterBuilder::new(format!("{:?} map", map.status()))
            .icon_url(format!("{AVATAR_URL}{}", map.creator_id()));

        let personal_best = personal
            .map(|top100| PersonalBestIndex::new(score, map.map_id(), map.status(), top100))
            .and_then(|pb_idx| pb_idx.into_embed_description(origin));

        let mut description = String::new();

        if pinned {
            description.push('📌');

            if personal_best.is_some() || global_idx <= GLOBAL_IDX_THRESHOLD {
                description.push(' ');
            } else {
                description.push('\u{200b}'); // zero-width character
            }
        }

        if personal_best.is_some() || global_idx <= GLOBAL_IDX_THRESHOLD {
            if personal_best.is_some() || global_idx <= 50 {
                description.push_str("__**");
            }

            if let Some(ref desc) = personal_best {
                description.push_str(desc);

                if global_idx <= GLOBAL_IDX_THRESHOLD {
                    description.reserve(19);
                    description.push_str(" and ");
                }
            }

            if global_idx <= GLOBAL_IDX_THRESHOLD {
                let _ = write!(description, "Global Top #{global_idx}");
            }

            if personal_best.is_some() || global_idx <= 50 {
                description.push_str("**__");
            }
        }

        let author = user.author_builder();
        let acc = round(entry.score.accuracy);
        let ago = HowLongAgoDynamic::new(&entry.score.ended_at);
        let timestamp = entry.score.ended_at;
        let mods = entry.score.mods;
        let score = WithComma::new(entry.score.score).to_string();

        Self {
            description,
            title,
            url: format!("{OSU_BASE}b/{}", map.map_id()),
            author,
            footer,
            timestamp,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id()),
            grade_completion_mods,
            stars: entry.stars,
            mode: entry.score.mode,
            score,
            acc,
            ago,
            pp: Some(entry.score.pp),
            max_pp: Some(*max_pp),
            combo,
            hits,
            mapset_id: map.mapset_id(),
            if_fc: if_fc.clone(),
            map_info: MapInfo::new(map, *stars).mods(mods).to_string(),
            minimized_pp,
        }
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
                    let _ = write!(result, "{pp:.2}");
                } else {
                    result.push('-');
                }

                if let Some(if_fc) = self.if_fc {
                    let _ = write!(result, "pp** ~~({:.2}pp)~~", if_fc.pp);
                } else {
                    result.push_str("**/");

                    if let Some(max) = self.max_pp {
                        let pp = self.pp.map(|pp| pp.max(max)).unwrap_or(max);
                        let _ = write!(result, "{pp:.2}");
                    } else {
                        result.push('-');
                    }

                    result.push_str("PP");
                }

                result
            }
            MinimizedPp::MaxPp => PpFormatter::new(self.pp, self.max_pp).to_string(),
        };

        let value = format!("{pp} [ {} ] {}", self.combo, self.hits);

        let mut title = self.title;
        let _ = write!(title, " [{}★]", round(self.stars));

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

        let pp = PpFormatter::new(self.pp, self.max_pp).to_string();
        let pp = highlight_funny_numeral(&pp).into_owned();

        let mut fields = fields![
            "Grade", self.grade_completion_mods.as_ref().to_owned(), true;
            "Score", score, true;
            "Acc", acc, true;
            "PP", pp, true;
        ];

        fields.reserve(3);

        let combo = highlight_funny_numeral(&self.combo).into_owned();
        let hits = self.hits.to_string();
        let hits = highlight_funny_numeral(&hits).into_owned();

        fields![fields {
            if self.mode == GameMode::Mania { "Combo / Ratio" } else { "Combo" }, combo, true;
            "Hits", hits, true;
        }];

        if let Some(ref if_fc) = self.if_fc {
            fields![fields {
                "**If FC**: PP", PpFormatter::new(Some(if_fc.pp), self.max_pp).to_string(), true;
                "Acc", format!("{}%", round(if_fc.accuracy())), true;
                "Hits", if_fc.hitresults().to_string(), true;
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
    pub fn new(user: &RedisData<User>, map: &OsuMap, mods: Option<ModSelection>) -> Self {
        let footer = FooterBuilder::new(format!("{:?} map by {}", map.status(), map.creator()))
            .icon_url(format!("{AVATAR_URL}{}", map.creator_id()));

        let title = format!(
            "{} - {} [{}]",
            map.artist().cow_escape_markdown(),
            map.title().cow_escape_markdown(),
            map.version().cow_escape_markdown()
        );

        let description = if mods.is_some() {
            "No scores with these mods"
        } else {
            "No scores"
        };

        Self {
            author: user.author_builder(),
            description,
            footer,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id()),
            title,
            url: format!("{OSU_BASE}b/{}", map.map_id()),
        }
    }
}
