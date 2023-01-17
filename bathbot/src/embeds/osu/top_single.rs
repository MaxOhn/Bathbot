use std::{borrow::Cow, fmt::Write};

use bathbot_psql::model::configs::MinimizedPp;
use bathbot_util::{
    constants::{AVATAR_URL, OSU_BASE},
    datetime::HowLongAgoDynamic,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder,
};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::TopEntry,
    core::Context,
    embeds::osu::PpFormatter,
    manager::redis::{osu::User, RedisData},
    util::osu::{grade_completion_mods, IfFc, MapInfo},
};

use super::{ComboFormatter, HitResultFormatter, KeyFormatter};

#[derive(Clone)]
pub struct TopSingleEmbed {
    description: String,
    title: String,
    url: String,
    author: AuthorBuilder,
    footer: FooterBuilder,
    timestamp: OffsetDateTime,
    thumbnail: String,

    stars: f32,
    mode: GameMode,
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
    mapset_cover: String,
    minimized_pp: MinimizedPp,
}

impl TopSingleEmbed {
    pub async fn new(
        user: &RedisData<User>,
        entry: &TopEntry,
        personal_idx: Option<usize>,
        global_idx: Option<usize>,
        minimized_pp: MinimizedPp,
        ctx: &Context,
    ) -> Self {
        let TopEntry {
            original_idx: _, // use personal_idx instead so this works for pinned aswell
            score,
            map,
            max_pp,
            stars,
        } = entry;

        let if_fc = IfFc::new(ctx, score, map).await;
        let hits = HitResultFormatter::new(score.mode, score.statistics.clone());
        let grade_completion_mods =
            grade_completion_mods(score.mods, score.grade, score.total_hits(), map);

        let (combo, title) = if score.mode == GameMode::Mania {
            let mut ratio = score.statistics.count_geki as f32;

            if score.statistics.count_300 > 0 {
                ratio /= score.statistics.count_300 as f32
            }

            let combo = format!("**{}x** / {ratio:.2}", &score.max_combo);

            let title = format!(
                "{} {} - {} [{}]",
                KeyFormatter::new(score.mods, map),
                map.artist().cow_escape_markdown(),
                map.title().cow_escape_markdown(),
                map.version().cow_escape_markdown(),
            );

            (combo, title)
        } else {
            let combo = ComboFormatter::new(score.max_combo, map.max_combo()).to_string();

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

        let description = if personal_idx.is_some() || global_idx.is_some() {
            let mut description = String::with_capacity(25);
            description.push_str("__**");

            if let Some(idx) = personal_idx {
                let _ = write!(description, "Personal Best #{}", idx + 1);

                if global_idx.is_some() {
                    description.reserve(19);
                    description.push_str(" and ");
                }
            }

            if let Some(idx) = global_idx {
                let _ = write!(description, "Global Top #{}", idx + 1);
            }

            description.push_str("**__");

            description
        } else {
            String::new()
        };

        Self {
            title,
            footer,
            thumbnail: map.thumbnail().to_owned(),
            description,
            url: format!("{OSU_BASE}b/{}", map.map_id()),
            author: user.author_builder(),
            timestamp: score.ended_at,
            grade_completion_mods,
            stars: *stars,
            mode: score.mode,
            score: WithComma::new(score.score).to_string(),
            acc: round(score.accuracy),
            ago: HowLongAgoDynamic::new(&score.ended_at),
            pp: Some(score.pp),
            max_pp: Some(*max_pp),
            combo,
            hits,
            map_info: MapInfo::new(map, *stars).mods(score.mods).to_string(),
            if_fc,
            mapset_cover: map.cover().to_owned(),
            minimized_pp,
        }
    }

    pub fn as_maximized(&self) -> Embed {
        let mut fields = fields![
            "Grade", self.grade_completion_mods.as_ref().to_owned(), true;
            "Score", self.score.clone(), true;
            "Acc", format!("{}%", self.acc), true;
            "PP", PpFormatter::new(self.pp, self.max_pp).to_string(), true;
        ];

        let mania = self.mode == GameMode::Mania;
        let combo_name = if mania { "Combo / Ratio" } else { "Combo" };

        fields![fields {
            combo_name, self.combo.clone(), true;
            "Hits", self.hits.to_string(), true;
        }];

        if let Some(ref if_fc) = self.if_fc {
            fields![fields {
                "**If FC**: PP", PpFormatter::new(Some(if_fc.pp), self.max_pp).to_string(), true;
                "Acc", format!("{}%", round(if_fc.accuracy())), true;
                "Hits", if_fc.hitresults().to_string(), true;
            }];
        }

        fields![fields { "Map Info", self.map_info.clone(), false }];

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

                if let Some(ref if_fc) = self.if_fc {
                    let _ = write!(result, "pp** ~~({:.2}pp)~~", if_fc.pp);
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
}
