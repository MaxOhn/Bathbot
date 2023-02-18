use std::{borrow::Cow, fmt::Write};

use bathbot_psql::model::configs::MinimizedPp;
use bathbot_util::{
    constants::{AVATAR_URL, OSU_BASE},
    datetime::HowLongAgoDynamic,
    matcher::highlight_funny_numeral,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder,
};
use osu::PpFormatter;
use rosu_v2::prelude::{BeatmapUserScore, GameMode, Score};
use time::OffsetDateTime;
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::RecentEntry,
    core::Context,
    embeds::osu,
    manager::redis::{osu::User, RedisData},
    util::osu::{grade_completion_mods, IfFc, MapInfo, PersonalBestIndex},
};

#[cfg(feature = "twitch")]
use bathbot_model::TwitchVideo;

use super::{ComboFormatter, HitResultFormatter, KeyFormatter, MessageOrigin};

pub struct RecentEmbed {
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
    #[cfg(feature = "twitch")]
    twitch_vod: Option<TwitchVideo>,
    minimized_pp: MinimizedPp,
}

impl RecentEmbed {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        user: &RedisData<User>,
        entry: &RecentEntry,
        personal: Option<&[Score]>,
        map_score: Option<&BeatmapUserScore>,
        #[cfg(feature = "twitch")] twitch_vod: Option<TwitchVideo>,
        minimized_pp: MinimizedPp,
        origin: &MessageOrigin,
        ctx: &Context,
    ) -> Self {
        let RecentEntry {
            score,
            map,
            max_pp,
            max_combo,
            stars,
        } = entry;

        let if_fc = IfFc::new(ctx, score, map).await;
        let hits = HitResultFormatter::new(score.mode, score.statistics.clone());
        let grade_completion_mods =
            grade_completion_mods(score.mods, score.grade, score.total_hits(), map);

        let (combo, title) = if map.mode() == GameMode::Mania {
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
            let combo = ComboFormatter::new(score.max_combo, Some(*max_combo)).to_string();

            let title = format!(
                "{} - {} [{}]",
                map.artist().cow_escape_markdown(),
                map.title().cow_escape_markdown(),
                map.version().cow_escape_markdown(),
            );

            (combo, title)
        };

        let footer = FooterBuilder::new(map.footer_text())
            .icon_url(format!("{AVATAR_URL}{}", map.creator_id()));

        let personal_best = personal
            .map(|top100| PersonalBestIndex::new(score, map.map_id(), map.status(), top100))
            .and_then(|pb_idx| pb_idx.into_embed_description(origin));

        let global_idx = map_score
            .and_then(|s| score.is_eq(s).then_some(s.pos))
            .filter(|&p| p <= 50);

        let description = if personal_best.is_some() || global_idx.is_some() {
            let mut description = String::with_capacity(25);
            description.push_str("__**");

            if let Some(desc) = personal_best {
                description.push_str(&desc);

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

        Self {
            description,
            title,
            url: format!("{OSU_BASE}b/{}", map.map_id()),
            author: user.author_builder(),
            footer,
            timestamp: score.ended_at,
            thumbnail: map.thumbnail().to_owned(),
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
            #[cfg(feature = "twitch")]
            twitch_vod,
        }
    }

    pub fn as_maximized(&self) -> Embed {
        let score = highlight_funny_numeral(&self.score).into_owned();
        let acc = highlight_funny_numeral(&format!("{}%", self.acc)).into_owned();

        let pp = PpFormatter::new(self.pp, self.max_pp).to_string();
        let pp = highlight_funny_numeral(&pp).into_owned();

        let mut fields = fields![
            "Grade",self.grade_completion_mods.as_ref().to_owned(), true;
            "Score", score, true;
            "Acc", acc, true;
            "PP", pp, true;
        ];

        fields.reserve(3 + (self.if_fc.is_some() as usize) * 3);

        let combo = highlight_funny_numeral(&self.combo).into_owned();
        let hits = self.hits.to_string();
        let hits = highlight_funny_numeral(&hits).into_owned();

        let mania = self.mode == GameMode::Mania;
        let name = if mania { "Combo / Ratio" } else { "Combo" };

        fields![fields {
            name, combo, true;
            "Hits", hits, true;
        }];

        if let Some(ref if_fc) = &self.if_fc {
            fields![fields {
                "**If FC**: PP", PpFormatter::new(Some(if_fc.pp), self.max_pp).to_string(), true;
                "Acc", format!("{}%", round(if_fc.accuracy())), true;
                "Hits", if_fc.hitresults().to_string(), true;
            }];
        }

        fields![fields { "Map Info".to_owned(), self.map_info.clone(), false }];

        #[cfg(feature = "twitch")]
        if let Some(ref vod) = self.twitch_vod {
            let twitch_channel = format!(
                "[**{name}**]({base}{name})",
                base = bathbot_util::constants::TWITCH_BASE,
                name = vod.username
            );

            let vod_hyperlink = format!("[**VOD**]({})", vod.url);

            fields![fields {
                "Live on twitch", twitch_channel, true;
                "Liveplay of this score", vod_hyperlink, true;
            }];
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

    pub fn into_minimized(#[allow(unused_mut)] mut self) -> Embed {
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

                if let Some(ref if_fc) = self.if_fc {
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

        let fields = fields![name, value, false];

        #[cfg(feature = "twitch")]
        if let Some(ref vod) = self.twitch_vod {
            let _ = write!(
                self.description,
                " {} [Liveplay on twitch]({})",
                crate::util::Emote::Twitch.text(),
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
