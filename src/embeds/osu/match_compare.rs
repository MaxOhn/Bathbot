use std::{borrow::Cow, cmp::Reverse, fmt::Write};

use command_macros::EmbedData;
use hashbrown::HashMap;
use rosu_v2::prelude::{Grade, Team, Username};
use twilight_model::channel::embed::EmbedField;

use crate::{
    commands::osu::{
        CommonMap, MatchCompareComparison, MatchCompareScore, ProcessedMatch, UniqueMap,
    },
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        numbers::{round, with_comma_int},
        osu::grade_emote,
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct MatchCompareMapEmbed {
    author: AuthorBuilder,
    footer: FooterBuilder,
    title: String,
    url: String,
    fields: Vec<EmbedField>,
}

impl MatchCompareMapEmbed {
    pub fn new(
        map: CommonMap,
        match_1: &str,
        match_2: &str,
        users: &HashMap<u32, Username>,
        comparison: MatchCompareComparison,
        (common_idx, common_total, maps_total): (usize, usize, usize),
    ) -> Self {
        let author_text = format!("Match compare - Common map {common_idx}/{common_total}");
        let author = AuthorBuilder::new(author_text);

        let footer_text = format!(
            "Page {common_idx}/{pages} | Common maps: {common_total}/{maps_total}",
            pages = common_total + 2,
        );

        let footer = FooterBuilder::new(footer_text);

        let match_1 = match_1.cow_escape_markdown();
        let match_2 = match_2.cow_escape_markdown();

        let fields = match comparison {
            MatchCompareComparison::Both => {
                let team_scores = team_scores(&map, &match_1, &match_2);

                fields![
                    match_1, prepare_scores(&map.match_1, map.match_1_scores, users, false), false;
                    match_2, prepare_scores(&map.match_2, map.match_2_scores, users, false), false;
                    "Total team scores", team_scores, false;
                ]
            }
            MatchCompareComparison::Players => {
                fields![
                    match_1, prepare_scores(&map.match_1, map.match_1_scores, users, true), false;
                    match_2, prepare_scores(&map.match_2, map.match_2_scores, users, true), false;
                ]
            }
            MatchCompareComparison::Teams => {
                fields![
                    "Total team scores",
                    team_scores(&map, &match_1, &match_2),
                    false
                ]
            }
        };

        let title = map.map;
        let url = format!("{OSU_BASE}b/{}", map.map_id);

        Self {
            author,
            footer,
            title,
            url,
            fields,
        }
    }
}

fn team_scores(map: &CommonMap, match_1: &str, match_2: &str) -> String {
    let mut scores = Vec::new();

    for team in [Team::Blue, Team::Red] {
        if map.match_1_scores[team as usize] > 0 {
            scores.push(TeamScore::new(
                team,
                match_1,
                map.match_1_scores[team as usize],
            ));
        }

        if map.match_2_scores[team as usize] > 0 {
            scores.push(TeamScore::new(
                team,
                match_2,
                map.match_2_scores[team as usize],
            ));
        }
    }

    if scores.is_empty() {
        return "No teams".to_owned();
    }

    scores.sort_unstable_by_key(|score| Reverse(score.score));

    let mut value = String::with_capacity(scores.len() * 80);

    for (score, i) in scores.into_iter().zip(1..) {
        let _ = writeln!(
            value,
            "**{i}.** `{score}` :{team}_circle:\n> {name}",
            score = with_comma_int(score.score),
            team = if score.team == Team::Blue {
                "blue"
            } else {
                "red"
            },
            name = score.name,
        );
    }

    value
}

struct TeamScore<'n> {
    team: Team,
    name: &'n str,
    score: u32,
}

impl<'n> TeamScore<'n> {
    fn new(team: Team, name: &'n str, score: u32) -> Self {
        Self { team, name, score }
    }
}

fn prepare_scores(
    scores: &[MatchCompareScore],
    totals: [u32; 3],
    users: &HashMap<u32, Username>,
    with_total: bool,
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

    if with_total && totals[1] + totals[2] > 0 {
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

#[derive(EmbedData)]
pub struct MatchCompareSummaryEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    title: String,
    url: String,
}

impl MatchCompareSummaryEmbed {
    pub fn new(
        common: &[CommonMap],
        processed: &ProcessedMatch,
        (page, common_maps, total_maps): (usize, usize, usize),
    ) -> Self {
        let author = AuthorBuilder::new("Match compare - Summary");
        let title = processed.name.to_owned();
        let url = format!("{OSU_BASE}mp/{}", processed.match_id);

        let footer_text = format!(
            "Page {page}/{pages} | Common maps: {common_maps}/{total_maps}",
            pages = common_maps + 2,
        );

        let footer = FooterBuilder::new(footer_text);

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
