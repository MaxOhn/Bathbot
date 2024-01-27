use std::fmt::Write;

use bathbot_model::{rosu_v2::user::User, ScoreSlim};
use bathbot_psql::model::configs::{MinimizedPp, ScoreSize};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    fields,
    matcher::highlight_funny_numeral,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, MessageOrigin,
};
use rosu_v2::prelude::{BeatmapUserScore, GameMode, Score};

use super::{ButtonData, EditOnTimeout, EditOnTimeoutKind};
#[cfg(feature = "twitch")]
use crate::commands::osu::RecentTwitchStream;
use crate::{
    active::BuildPage,
    commands::osu::RecentEntry,
    core::Context,
    embeds::{ComboFormatter, HitResultFormatter, KeyFormatter, PpFormatter},
    manager::{redis::RedisData, OsuMap, OwnedReplayScore},
    util::{
        osu::{grade_completion_mods, IfFc, MapInfo, PersonalBestIndex},
        Emote,
    },
};

pub struct RecentScoreEdit {
    pub(super) button_data: ButtonData,
}

impl RecentScoreEdit {
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        ctx: &Context,
        user: &RedisData<User>,
        entry: &RecentEntry,
        personal: Option<&[Score]>,
        map_score: Option<&BeatmapUserScore>,
        #[cfg(feature = "twitch")] twitch_stream: Option<RecentTwitchStream>,
        minimized_pp: MinimizedPp,
        score_id: Option<u64>,
        with_miss_analyzer_button: bool,
        replay_score: Option<OwnedReplayScore>,
        origin: &MessageOrigin,
        size: ScoreSize,
        content: Option<String>,
    ) -> EditOnTimeout {
        let RecentEntry {
            score,
            map,
            max_pp,
            max_combo,
            stars,
        } = entry;

        let if_fc = IfFc::new(ctx, score, map).await;

        let (combo, title) = if score.mode == GameMode::Mania {
            let mut ratio = score.statistics.count_geki as f32;

            if score.statistics.count_300 > 0 {
                ratio /= score.statistics.count_300 as f32
            }

            let combo = format!("**{}x** / {ratio:.2}", &score.max_combo);

            let title = format!(
                "{} {} - {} [{}]",
                KeyFormatter::new(&score.mods, map.cs()),
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

        let url = format!("{OSU_BASE}b/{}", map.map_id());
        let author = user.author_builder();
        let pp = Some(score.pp);
        let max_pp = Some(*max_pp);

        let kind = Self {
            button_data: ButtonData {
                score_id,
                with_miss_analyzer_button,
                replay_score,
            },
        };

        match size {
            ScoreSize::AlwaysMinimized => {
                let minimized = Self::minimized(
                    score,
                    map,
                    *stars,
                    pp,
                    max_pp,
                    if_fc.as_ref(),
                    combo,
                    minimized_pp,
                    author,
                    description,
                    title,
                    url,
                    #[cfg(feature = "twitch")]
                    twitch_stream.as_ref(),
                );

                let mut build = BuildPage::new(minimized, false);

                if let Some(content) = content {
                    build = build.content(content);
                }

                EditOnTimeout::new_stay(build, kind)
            }
            ScoreSize::AlwaysMaximized => {
                let maximized = Self::maximized(
                    score,
                    map,
                    *stars,
                    pp,
                    max_pp,
                    if_fc.as_ref(),
                    combo,
                    author,
                    description,
                    title,
                    url,
                    #[cfg(feature = "twitch")]
                    twitch_stream.as_ref(),
                );

                let mut build = BuildPage::new(maximized, false);

                if let Some(content) = content {
                    build = build.content(content);
                }

                EditOnTimeout::new_stay(build, kind)
            }
            ScoreSize::InitialMaximized => {
                let minimized = Self::minimized(
                    score,
                    map,
                    *stars,
                    pp,
                    max_pp,
                    if_fc.as_ref(),
                    combo.clone(),
                    minimized_pp,
                    author.clone(),
                    description.clone(),
                    title.clone(),
                    url.clone(),
                    #[cfg(feature = "twitch")]
                    twitch_stream.as_ref(),
                );
                let maximized = Self::maximized(
                    score,
                    map,
                    *stars,
                    pp,
                    max_pp,
                    if_fc.as_ref(),
                    combo,
                    author,
                    description,
                    title,
                    url,
                    #[cfg(feature = "twitch")]
                    twitch_stream.as_ref(),
                );

                let mut edited = BuildPage::new(minimized, false);

                if let Some(content) = content.clone() {
                    edited = edited.content(content);
                }

                let mut initial = BuildPage::new(maximized, false);

                if let Some(content) = content {
                    initial = initial.content(content);
                }

                EditOnTimeout::new_edit(initial, edited, kind)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn minimized(
        score: &ScoreSlim,
        map: &OsuMap,
        stars: f32,
        pp: Option<f32>,
        max_pp: Option<f32>,
        if_fc: Option<&IfFc>,
        combo: String,
        minimized_pp: MinimizedPp,
        author: AuthorBuilder,
        #[allow(unused_mut)] // feature-gated
        mut description: String,
        mut title: String,
        url: String,
        #[cfg(feature = "twitch")] twitch_stream: Option<&RecentTwitchStream>,
    ) -> EmbedBuilder {
        let name = format!(
            "{grade_completion_mods}\t{score}\t({acc}%)\t{ago}",
            grade_completion_mods = grade_completion_mods(
                &score.mods,
                score.grade,
                score.total_hits(),
                map.mode(),
                map.n_objects() as u32
            ),
            score = WithComma::new(score.score),
            acc = round(score.accuracy),
            ago = HowLongAgoDynamic::new(&score.ended_at),
        );

        let pp = match minimized_pp {
            MinimizedPp::IfFc => {
                let mut result = String::with_capacity(17);
                result.push_str("**");

                if let Some(pp) = pp {
                    let _ = write!(result, "{pp:.2}");
                } else {
                    result.push('-');
                }

                if let Some(if_fc) = if_fc {
                    let _ = write!(result, "pp** ~~({:.2}pp)~~", if_fc.pp);
                } else {
                    result.push_str("**/");

                    if let Some(max) = max_pp {
                        let pp = pp.map(|pp| pp.max(max)).unwrap_or(max);
                        let _ = write!(result, "{pp:.2}");
                    } else {
                        result.push('-');
                    }

                    result.push_str("PP");
                }

                result
            }
            MinimizedPp::MaxPp => PpFormatter::new(pp, max_pp).to_string(),
        };

        let value = format!(
            "{pp} [ {combo} ] {hits}",
            hits = HitResultFormatter::new(score.mode, score.statistics.clone())
        );

        let _ = write!(title, " [{}★]", round(stars));

        let fields = fields![name, value, false];

        #[cfg(feature = "twitch")]
        match twitch_stream {
            Some(RecentTwitchStream::Stream { login }) => {
                let _ = write!(
                    description,
                    " {emote} [Streaming on twitch]({base}{login})",
                    emote = Emote::Twitch,
                    base = bathbot_util::constants::TWITCH_BASE,
                );
            }
            Some(RecentTwitchStream::Video { vod_url, .. }) => {
                let _ = write!(
                    description,
                    " {emote} [Liveplay on twitch]({vod_url})",
                    emote = Emote::Twitch,
                );
            }
            None => {}
        }

        EmbedBuilder::new()
            .author(author)
            .description(description)
            .fields(fields)
            .thumbnail(map.thumbnail())
            .title(title)
            .url(url)
    }

    #[allow(clippy::too_many_arguments)]
    fn maximized(
        score: &ScoreSlim,
        map: &OsuMap,
        stars: f32,
        pp: Option<f32>,
        max_pp: Option<f32>,
        if_fc: Option<&IfFc>,
        combo: String,
        author: AuthorBuilder,
        #[allow(unused_mut)] // feature-gated
        mut description: String,
        title: String,
        url: String,
        #[cfg(feature = "twitch")] twitch_stream: Option<&RecentTwitchStream>,
    ) -> EmbedBuilder {
        let mut score_str = WithComma::new(score.score).to_string();
        score_str = highlight_funny_numeral(&score_str).into_owned();

        let acc = round(score.accuracy);
        let acc = highlight_funny_numeral(&format!("{acc}%")).into_owned();

        let pp = PpFormatter::new(pp, max_pp).to_string();
        let pp = highlight_funny_numeral(&pp).into_owned();

        let grade_completion_mods = grade_completion_mods(
            &score.mods,
            score.grade,
            score.total_hits(),
            map.mode(),
            map.n_objects() as u32,
        )
        .into_owned();

        let mut fields = fields![
            "Grade", grade_completion_mods, true;
            "Score", score_str, true;
            "Acc", acc, true;
            "PP", pp, true;
        ];

        fields.reserve(3 + (if_fc.is_some() as usize) * 3);

        let combo = highlight_funny_numeral(&combo).into_owned();

        let mut hits = HitResultFormatter::new(score.mode, score.statistics.clone()).to_string();
        hits = highlight_funny_numeral(&hits).into_owned();

        let mania = score.mode == GameMode::Mania;
        let name = if mania { "Combo / Ratio" } else { "Combo" };

        fields![fields {
            name, combo, true;
            "Hits", hits, true;
        }];

        if let Some(if_fc) = if_fc {
            fields![fields {
                "**If FC**: PP", PpFormatter::new(Some(if_fc.pp), max_pp).to_string(), true;
                "Acc", format!("{}%", round(if_fc.accuracy())), true;
                "Hits", if_fc.hitresults().to_string(), true;
            }];
        }

        let map_info = MapInfo::new(map, stars).mods(score.mods.bits()).to_string();
        fields![fields { "Map Info".to_owned(), map_info, false }];

        #[cfg(feature = "twitch")]
        match twitch_stream {
            Some(RecentTwitchStream::Stream { login }) => {
                let _ = write!(
                    description,
                    " {emote} [Streaming on twitch]({base}{login})",
                    emote = Emote::Twitch,
                    base = bathbot_util::constants::TWITCH_BASE,
                );
            }
            Some(RecentTwitchStream::Video {
                username,
                login,
                vod_url,
            }) => {
                let twitch_channel = format!(
                    "[**{username}**]({base}{login})",
                    base = bathbot_util::constants::TWITCH_BASE,
                );

                let vod_hyperlink = format!("[**VOD**]({vod_url})");

                fields![fields {
                    "Live on twitch", twitch_channel, true;
                    "Liveplay of this score", vod_hyperlink, true;
                }];
            }
            None => {}
        }

        let footer = FooterBuilder::new(map.footer_text()).icon_url(Emote::from(score.mode).url());

        EmbedBuilder::new()
            .author(author)
            .description(description)
            .fields(fields)
            .footer(footer)
            .image(map.cover())
            .timestamp(score.ended_at)
            .title(title)
            .url(url)
    }
}

impl From<RecentScoreEdit> for EditOnTimeoutKind {
    fn from(recent_score: RecentScoreEdit) -> Self {
        EditOnTimeoutKind::RecentScore(recent_score)
    }
}
