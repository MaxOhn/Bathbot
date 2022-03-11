use std::{borrow::Cow, fmt::Write};

use hashbrown::HashMap;
use rosu_v2::prelude::{Grade, Team, Username};

use crate::{
    commands::osu::{CommonMap, MatchCompareScore, ProcessedMatch, UniqueMap},
    embeds::{Author, EmbedFields, Footer},
    util::{
        constants::OSU_BASE,
        numbers::{round, with_comma_int},
        osu::grade_emote,
    },
};

pub struct MatchCompareMapEmbed {
    author: Author,
    footer: Footer,
    title: String,
    url: String,
    fields: EmbedFields,
}

impl MatchCompareMapEmbed {
    pub fn new(
        map: CommonMap,
        match_1: &str,
        match_2: &str,
        users: &HashMap<u32, Username>,
        (common_idx, common_total, maps_total): (usize, usize, usize),
    ) -> Self {
        let author_text = format!("Match compare - Common map {common_idx}/{common_total}");
        let author = Author::new(author_text);

        let title = map.map;
        let url = format!("{OSU_BASE}b/{}", map.map_id);

        let footer_text = format!(
            "Page {common_idx}/{pages} | Common maps: {common_total}/{maps_total}",
            pages = common_total + 2,
        );

        let footer = Footer::new(footer_text);

        let fields = vec![
            field!(
                match_1,
                prepare_scores(&map.match_1, map.match_1_scores, users),
                false
            ),
            field!(
                match_2,
                prepare_scores(&map.match_2, map.match_2_scores, users),
                false
            ),
        ];

        Self {
            author,
            footer,
            title,
            url,
            fields,
        }
    }
}

fn prepare_scores(
    scores: &[MatchCompareScore],
    totals: [u32; 3],
    users: &HashMap<u32, Username>,
) -> String {
    let mut embed_scores = Vec::with_capacity(scores.len());
    let mut sizes = ColumnSizes::default();

    let iter = scores.iter().filter(|score| score.score > 0).map(|score| {
        let name = match users.get(&score.user_id) {
            Some(name) => Cow::Borrowed(name.as_str()),
            None => format!("`User id {}`", score.user_id).into(),
        };

        let score_str = with_comma_int(score.score).to_string();
        let combo = with_comma_int(score.combo).to_string();
        let mods = score.mods.to_string();

        sizes.name = sizes.name.max(name.len());
        sizes.combo = sizes.combo.max(combo.len());
        sizes.score = sizes.score.max(score_str.len());
        sizes.mods = sizes.mods.max(mods.len());

        EmbedScore {
            username: name,
            mods,
            accuracy: score.acc,
            grade: score.grade,
            combo,
            score_str,
            team: score.team,
        }
    });

    // Collect iter so that `sizes` is correct
    embed_scores.extend(iter);

    let mut value = String::new();

    if totals[1] + totals[2] > 0 {
        let _ = writeln!(
            value,
            "**Total**: :blue_circle: {blue_won}{blue_score}{blue_won} \
            - {red_won}{red_score}{red_won} :red_circle:",
            blue_score = with_comma_int(totals[1]),
            red_score = with_comma_int(totals[2]),
            blue_won = if totals[1] > totals[2] { "**" } else { "" },
            red_won = if totals[2] > totals[1] { "**" } else { "" },
        );
    }

    for score in embed_scores {
        let _ = write!(
            value,
            "{grade} `{name:<name_len$}` `+{mods:<mods_len$}` `{acc:>5}%` \
            `{combo:>combo_len$}x` `{score:>score_len$}`",
            grade = grade_emote(score.grade),
            name = score.username,
            name_len = sizes.name,
            mods = score.mods,
            mods_len = sizes.mods,
            acc = round(score.accuracy),
            combo = score.combo,
            combo_len = sizes.combo,
            score = score.score_str,
            score_len = sizes.score,
        );

        match score.team {
            Team::None => {}
            Team::Blue => value.push_str(" :blue_circle:"),
            Team::Red => value.push_str(" :red_circle:"),
        }

        value.push('\n');
    }

    if value.is_empty() {
        value.push_str("No scores");
    }

    value
}

struct EmbedScore<'n> {
    username: Cow<'n, str>,
    mods: String,
    accuracy: f32,
    grade: Grade,
    combo: String,
    score_str: String,
    team: Team,
}

#[derive(Default)]
struct ColumnSizes {
    name: usize,
    combo: usize,
    score: usize,
    mods: usize,
}

impl_builder!(MatchCompareMapEmbed {
    author,
    footer,
    title,
    url,
    fields,
});

pub struct MatchCompareSummaryEmbed {
    author: Author,
    description: String,
    footer: Footer,
    title: String,
    url: String,
}

impl MatchCompareSummaryEmbed {
    pub fn new<'m>(
        common: &[CommonMap],
        processed: &ProcessedMatch,
        (page, common_maps, total_maps): (usize, usize, usize),
    ) -> Self {
        let author = Author::new("Match compare - Summary");
        let title = processed.name.to_owned();
        let url = format!("{OSU_BASE}mp/{}", processed.match_id);

        let footer_text = format!(
            "Page {page}/{pages} | Common maps: {common_maps}/{total_maps}",
            pages = common_maps + 2,
        );

        let footer = Footer::new(footer_text);

        let mut description = String::new();

        description.push_str("__Common maps in both matches:__\n");

        for CommonMap { map, map_id, .. } in common {
            let _ = writeln!(description, "- [{map}]({OSU_BASE}b/{map_id})",);
        }

        description.push_str("\n__Maps of this match but not the other:__\n");

        for UniqueMap { map, map_id } in processed.unique_maps.iter() {
            let _ = writeln!(description, "- [{map}]({OSU_BASE}b/{map_id})");
        }

        Self {
            author,
            description,
            footer,
            title,
            url,
        }
    }
}

impl_builder!(MatchCompareSummaryEmbed {
    author,
    description,
    footer,
    title,
    url,
});
