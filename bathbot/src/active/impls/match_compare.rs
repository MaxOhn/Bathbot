use std::{borrow::Cow, cmp::Reverse, collections::HashMap, fmt::Write, mem};

use bathbot_util::{
    constants::OSU_BASE,
    fields,
    numbers::{round, WithComma},
    osu::calculate_grade,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, IntHasher,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::prelude::{
    Beatmap, GameMode, GameModsIntermode, Grade, MatchEvent, MatchGame, MatchScore, OsuMatch, Team,
    Username,
};
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::MatchCompareComparison,
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::grade_emote,
    },
};

pub struct MatchComparePagination {
    embeds: Vec<EmbedBuilder>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MatchComparePagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let embed = self.embeds[self.pages.index()].clone();

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(modal, self.msg_owner, false, &mut self.pages)
    }
}

impl MatchComparePagination {
    pub fn new(
        match1: &mut OsuMatch,
        match2: &mut OsuMatch,
        comparison: MatchCompareComparison,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        let users: HashMap<_, _, IntHasher> = match1
            .users
            .drain()
            .chain(match2.users.drain())
            .map(|(user_id, user)| (user_id, user.username))
            .collect();

        match1
            .events
            .retain(|event| matches!(event, MatchEvent::Game { .. }));

        match2
            .events
            .retain(|event| matches!(event, MatchEvent::Game { .. }));

        let mut processed1 = ProcessedMatch::new(
            mem::take(&mut match1.name).into_boxed_str(),
            match1.match_id,
        );
        let mut processed2 = ProcessedMatch::new(
            mem::take(&mut match2.name).into_boxed_str(),
            match2.match_id,
        );

        let mut common_maps = Vec::new();

        for mut game_1 in match1.drain_games() {
            let (map_id, map) = match game_1.map.as_ref().filter(|_| game_1.end_time.is_some()) {
                Some(map) => (map.map_id, map_name(map)),
                None => continue,
            };

            let idx = match2.events.iter().position(|event| match event {
                MatchEvent::Game { game, .. } => game
                    .map
                    .as_ref()
                    .filter(|_| game.end_time.is_some())
                    .map(|m| m.map_id == map_id)
                    .unwrap_or(false),
                _ => unreachable!(),
            });

            if let Some(idx) = idx {
                let mut game_2 = match match2.events.remove(idx) {
                    MatchEvent::Game { game, .. } => *game,
                    _ => unreachable!(),
                };

                common_maps.push(CommonMap::new(map, map_id, &mut game_1, &mut game_2));
            } else {
                processed1.unique_maps.push(UniqueMap::new(map, map_id));
            }
        }

        for game in match2.drain_games() {
            let (map_id, map) = match game.map.as_ref().filter(|_| game.end_time.is_some()) {
                Some(map) => (map.map_id, map_name(map)),
                None => continue,
            };

            processed2.unique_maps.push(UniqueMap::new(map, map_id));
        }

        let mut embeds = Vec::with_capacity(common_maps.len() + 2);
        let common_total = common_maps.len();

        let maps_total =
            common_maps.len() + processed1.unique_maps.len() + processed2.unique_maps.len();

        let tuple = (common_total + 1, common_total, maps_total);
        let summary1 = Self::summary_embed(&common_maps, &processed1, tuple);

        let tuple = (common_total + 2, common_total, maps_total);
        let summary2 = Self::summary_embed(&common_maps, &processed2, tuple);

        let iter = common_maps.into_iter().zip(1..).map(|(map, i)| {
            Self::compare_map_embed(
                map,
                &processed1.name,
                &processed2.name,
                &users,
                comparison,
                (i, common_total, maps_total),
            )
        });

        embeds.extend(iter);

        embeds.push(summary1);
        embeds.push(summary2);

        let pages = Pages::new(1, embeds.len());

        Self {
            embeds,
            msg_owner,
            pages,
        }
    }

    pub fn into_embeds(self) -> Vec<EmbedBuilder> {
        self.embeds
    }

    fn compare_map_embed(
        map: CommonMap,
        match1: &str,
        match2: &str,
        users: &HashMap<u32, Username, IntHasher>,
        comparison: MatchCompareComparison,
        (common_idx, common_total, maps_total): (usize, usize, usize),
    ) -> EmbedBuilder {
        let author_text = format!("Match compare - Common map {common_idx}/{common_total}");
        let author = AuthorBuilder::new(author_text);

        let footer_text = format!(
            "Page {common_idx}/{pages} • Common maps: {common_total}/{maps_total}",
            pages = common_total + 2,
        );

        let footer = FooterBuilder::new(footer_text);

        let match1 = match1.cow_escape_markdown();
        let match2 = match2.cow_escape_markdown();

        let fields = match comparison {
            MatchCompareComparison::Both => {
                let team_scores = Self::team_scores(&map, &match1, &match2);

                fields![
                    match1, Self::format_scores(&map.match1, map.match1_scores, users, false), false;
                    match2, Self::format_scores(&map.match2, map.match2_scores, users, false), false;
                    "Total team scores", team_scores, false;
                ]
            }
            MatchCompareComparison::Players => {
                fields![
                    match1, Self::format_scores(&map.match1, map.match1_scores, users, true), false;
                    match2, Self::format_scores(&map.match2, map.match2_scores, users, true), false;
                ]
            }
            MatchCompareComparison::Teams => {
                fields![
                    "Total team scores",
                    Self::team_scores(&map, &match1, &match2),
                    false
                ]
            }
        };

        let title = map.map;
        let url = format!("{OSU_BASE}b/{}", map.map_id);

        EmbedBuilder::new()
            .author(author)
            .fields(fields)
            .footer(footer)
            .title(title)
            .url(url)
    }

    fn team_scores(map: &CommonMap, match1: &str, match2: &str) -> String {
        struct TeamScore<'n> {
            team: Team,
            name: &'n str,
            score: u32,
        }

        let mut scores = Vec::new();

        for team in [Team::Blue, Team::Red] {
            if map.match1_scores[team as usize] > 0 {
                scores.push(TeamScore {
                    team,
                    name: match1,
                    score: map.match1_scores[team as usize],
                });
            }

            if map.match2_scores[team as usize] > 0 {
                scores.push(TeamScore {
                    team,
                    name: match2,
                    score: map.match2_scores[team as usize],
                });
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
                "**#{i}** `{score}` :{team}_circle:\n\
                {name}",
                score = WithComma::new(score.score),
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

    fn format_scores(
        scores: &[MatchCompareScore],
        totals: [u32; 3],
        users: &HashMap<u32, Username, IntHasher>,
        with_total: bool,
    ) -> String {
        let mut embed_scores = Vec::with_capacity(scores.len());
        let mut sizes = ColumnSizes::default();

        let iter = scores.iter().filter(|score| score.score > 0).map(|score| {
            let name = match users.get(&score.user_id) {
                Some(name) => Cow::Borrowed(name.as_str()),
                None => format!("`User id {}`", score.user_id).into(),
            };

            let score_str = WithComma::new(score.score).to_string().into_boxed_str();
            let combo = WithComma::new(score.combo).to_string().into_boxed_str();
            let mods = score.mods.to_string().into_boxed_str();

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
                blue_score = WithComma::new(totals[1]),
                red_score = WithComma::new(totals[2]),
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

    fn summary_embed(
        common: &[CommonMap],
        processed: &ProcessedMatch,
        (page, common_maps, total_maps): (usize, usize, usize),
    ) -> EmbedBuilder {
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

        EmbedBuilder::new()
            .author(author)
            .description(description)
            .footer(footer)
            .title(title)
            .url(url)
    }
}

pub struct CommonMap {
    pub map: Box<str>,
    pub map_id: u32,
    pub match1: Box<[MatchCompareScore]>,
    pub match1_scores: [u32; 3],
    pub match2: Box<[MatchCompareScore]>,
    pub match2_scores: [u32; 3],
}

impl CommonMap {
    fn new(map: Box<str>, map_id: u32, game1: &mut MatchGame, game2: &mut MatchGame) -> Self {
        let mut match1_scores = [0; 3];

        let mut match1: Box<[_]> = game1
            .scores
            .drain(..)
            .map(|score| MatchCompareScore::new(score, game1.mode))
            .inspect(|score| match1_scores[score.team as usize] += score.score)
            .collect();

        let mut match2_scores = [0; 3];

        let mut match2: Box<[_]> = game2
            .scores
            .drain(..)
            .map(|score| MatchCompareScore::new(score, game2.mode))
            .inspect(|score| match2_scores[score.team as usize] += score.score)
            .collect();

        let score_compare = |a: &MatchCompareScore, b: &MatchCompareScore| {
            (a.team as u8)
                .cmp(&(b.team as u8))
                .then_with(|| b.score.cmp(&a.score))
        };

        match1.sort_unstable_by(score_compare);
        match2.sort_unstable_by(score_compare);

        Self {
            map,
            map_id,
            match1,
            match1_scores,
            match2,
            match2_scores,
        }
    }
}

pub struct MatchCompareScore {
    pub grade: Grade,
    pub user_id: u32,
    pub mods: GameModsIntermode,
    pub acc: f32,
    pub combo: u32,
    pub score: u32,
    pub team: Team,
}

impl MatchCompareScore {
    fn new(score: MatchScore, mode: GameMode) -> Self {
        Self {
            grade: calculate_grade(mode, &score.mods, &score.statistics),
            user_id: score.user_id,
            mods: score.mods,
            acc: score.accuracy,
            combo: score.max_combo,
            score: score.score,
            team: score.team,
        }
    }
}

pub struct ProcessedMatch {
    pub name: Box<str>,
    pub match_id: u32,
    pub unique_maps: Vec<UniqueMap>,
}

impl ProcessedMatch {
    fn new(name: Box<str>, match_id: u32) -> Self {
        let name = match name.cow_escape_markdown() {
            Cow::Borrowed(_) => name,
            Cow::Owned(owned) => owned.into_boxed_str(),
        };

        Self {
            name,
            match_id,
            unique_maps: Vec::new(),
        }
    }
}

pub struct UniqueMap {
    pub map: Box<str>,
    pub map_id: u32,
}

impl UniqueMap {
    fn new(map: Box<str>, map_id: u32) -> Self {
        Self { map, map_id }
    }
}

fn map_name(map: &Beatmap) -> Box<str> {
    let mut name = String::new();

    if let Some(ref mapset) = map.mapset {
        name.push_str(mapset.title.cow_escape_markdown().as_ref());
    } else {
        name.push_str("<unknown title>")
    }

    let _ = write!(name, " [{}]", map.version.cow_escape_markdown());

    name.into_boxed_str()
}

struct EmbedScore<'n> {
    username: Cow<'n, str>,
    mods: Box<str>,
    accuracy: f32,
    grade: Grade,
    combo: Box<str>,
    score_str: Box<str>,
    team: Team,
}

#[derive(Default)]
struct ColumnSizes {
    name: usize,
    combo: usize,
    score: usize,
    mods: usize,
}
