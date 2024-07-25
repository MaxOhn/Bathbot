use std::fmt::Write;

use bathbot_model::{rosu_v2::user::User, ScoreSlim};
use bathbot_psql::model::configs::{MinimizedPp, ScoreData, ScoreSize};
use bathbot_util::{
    constants::OSU_BASE, datetime::HowLongAgoDynamic, fields, numbers::round, AuthorBuilder,
    CowUtils, EmbedBuilder, FooterBuilder,
};
use rosu_v2::prelude::GameMode;

use super::{ButtonData, EditOnTimeout, EditOnTimeoutKind};
use crate::{
    active::BuildPage,
    commands::osu::TopEntry,
    embeds::{ComboFormatter, HitResultFormatter, KeyFormatter, PpFormatter},
    manager::{redis::RedisData, OsuMap, OwnedReplayScore},
    util::{
        osu::{GradeCompletionFormatter, IfFc, MapInfo, ScoreFormatter},
        Emote,
    },
};

pub struct TopScoreEdit {
    pub(super) button_data: ButtonData,
}

impl TopScoreEdit {
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        user: &RedisData<User>,
        entry: &TopEntry,
        personal_idx: Option<usize>,
        global_idx: Option<usize>,
        minimized_pp: MinimizedPp,
        score_id: Option<u64>,
        replay_score: Option<OwnedReplayScore>,
        size: ScoreSize,
        score_data: ScoreData,
        content: Option<String>,
    ) -> EditOnTimeout {
        let TopEntry {
            original_idx: _, // use personal_idx instead so this works for pinned aswell
            score,
            map,
            max_pp,
            max_combo,
            stars,
            replay: _,
        } = entry;

        let if_fc = IfFc::new(score, map).await;

        let (combo, title) = if score.mode == GameMode::Mania {
            let mut ratio = score.statistics.count_geki as f32;

            if score.statistics.count_300 > 0 {
                ratio /= score.statistics.count_300 as f32
            }

            let combo = format!("**{}x** / {ratio:.2}", &score.max_combo);

            let title = format!(
                "{} {} - {} [{}]",
                KeyFormatter::new(&score.mods, map.attributes().build().cs as f32),
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

        let footer = FooterBuilder::new(map.footer_text()).icon_url(Emote::from(score.mode).url());

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

        let url = format!("{OSU_BASE}b/{}", map.map_id());
        let author = user.author_builder();

        let kind = Self {
            button_data: ButtonData {
                score_id,
                with_miss_analyzer_button: false,
                replay_score,
            },
        };

        let score_fmt = ScoreFormatter::new(score, score_data);

        match size {
            ScoreSize::AlwaysMinimized => {
                let minimized = Self::minimized(
                    score,
                    map,
                    *stars,
                    *max_pp,
                    if_fc.as_ref(),
                    combo,
                    score_fmt,
                    minimized_pp,
                    author,
                    description,
                    footer,
                    title,
                    url,
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
                    *max_pp,
                    if_fc.as_ref(),
                    combo,
                    score_fmt,
                    author,
                    description,
                    footer,
                    title,
                    url,
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
                    *max_pp,
                    if_fc.as_ref(),
                    combo.clone(),
                    score_fmt,
                    minimized_pp,
                    author.clone(),
                    description.clone(),
                    footer.clone(),
                    title.clone(),
                    url.clone(),
                );

                let maximized = Self::maximized(
                    score,
                    map,
                    *stars,
                    *max_pp,
                    if_fc.as_ref(),
                    combo,
                    score_fmt,
                    author,
                    description,
                    footer,
                    title,
                    url,
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
        max_pp: f32,
        if_fc: Option<&IfFc>,
        combo: String,
        score_fmt: ScoreFormatter,
        minimized_pp: MinimizedPp,
        author: AuthorBuilder,
        description: String,
        footer: FooterBuilder,
        mut title: String,
        url: String,
    ) -> EmbedBuilder {
        let name = format!(
            "{grade_completion_mods}\t{score_fmt}\t({acc}%)\t{ago}",
            // We don't use `GradeCompletionFormatter::new` so that it doesn't
            // use the score id to hyperlink the grade because those don't
            // work in embed field names.
            grade_completion_mods = GradeCompletionFormatter::new_without_score(
                &score.mods,
                score.grade,
                score.total_hits(),
                map.mode(),
                map.n_objects()
            ),
            acc = round(score.accuracy),
            ago = HowLongAgoDynamic::new(&score.ended_at),
        );

        let pp = match minimized_pp {
            MinimizedPp::IfFc => {
                let mut result = String::with_capacity(17);
                result.push_str("**");

                let _ = write!(result, "{:.2}", score.pp);

                if let Some(if_fc) = if_fc {
                    let _ = write!(result, "pp** ~~({:.2}pp)~~", if_fc.pp);
                } else {
                    result.push_str("**/");

                    let pp = score.pp.max(max_pp);
                    let _ = write!(result, "{pp:.2}");

                    result.push_str("PP");
                }

                result
            }
            MinimizedPp::MaxPp => PpFormatter::new(Some(score.pp), Some(max_pp)).to_string(),
        };

        let value = format!(
            "{pp} [ {combo} ] {hits}",
            hits = HitResultFormatter::new(score.mode, score.statistics.clone())
        );

        let _ = write!(title, " [{}★]", round(stars));

        let fields = fields![name, value, false];

        EmbedBuilder::new()
            .author(author)
            .description(description)
            .fields(fields)
            .footer(footer)
            .thumbnail(map.thumbnail())
            .title(title)
            .url(url)
    }

    #[allow(clippy::too_many_arguments)]
    fn maximized(
        score: &ScoreSlim,
        map: &OsuMap,
        stars: f32,
        max_pp: f32,
        if_fc: Option<&IfFc>,
        combo: String,
        score_fmt: ScoreFormatter,
        author: AuthorBuilder,
        description: String,
        footer: FooterBuilder,
        title: String,
        url: String,
    ) -> EmbedBuilder {
        let score_str = score_fmt.to_string();
        let acc = format!("{}%", round(score.accuracy));
        let pp = PpFormatter::new(Some(score.pp), Some(max_pp)).to_string();

        let grade_completion_mods =
            GradeCompletionFormatter::new(score, map.mode(), map.n_objects()).to_string();

        let mut fields = fields![
            "Grade", grade_completion_mods, true;
            "Score", score_str, true;
            "Acc", acc, true;
            "PP", pp, true;
        ];

        fields.reserve(3 + (if_fc.is_some() as usize) * 3);

        let hits = HitResultFormatter::new(score.mode, score.statistics.clone()).to_string();

        let mania = score.mode == GameMode::Mania;
        let combo_name = if mania { "Combo / Ratio" } else { "Combo" };

        fields![fields {
            combo_name, combo, true;
            "Hits", hits, true;
        }];

        if let Some(if_fc) = if_fc {
            fields![fields {
                "**If FC**: PP", PpFormatter::new(Some(if_fc.pp), Some(max_pp)).to_string(), true;
                "Acc", format!("{}%", round(if_fc.accuracy())), true;
                "Hits", if_fc.hitresults().to_string(), true;
            }];
        }

        let map_info = MapInfo::new(map, stars).mods(&score.mods).to_string();
        fields![fields { "Map Info", map_info, false }];

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

impl From<TopScoreEdit> for EditOnTimeoutKind {
    fn from(top_score: TopScoreEdit) -> Self {
        EditOnTimeoutKind::TopScore(top_score)
    }
}
