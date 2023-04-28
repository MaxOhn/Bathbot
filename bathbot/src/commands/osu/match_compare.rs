use std::{borrow::Cow, fmt::Write, mem, sync::Arc, time::Duration};

use bathbot_macros::SlashCommand;
use bathbot_util::{
    constants::OSU_API_ISSUE, matcher, osu::calculate_grade, CowUtils, IntHasher, MessageBuilder,
};
use eyre::{Report, Result};
use hashbrown::HashMap;
use rosu_v2::prelude::{
    BeatmapCompact, GameMode, GameModsIntermode, Grade, MatchEvent, MatchGame, MatchScore,
    OsuError, OsuMatch, Team, Username,
};
use tokio::time::interval;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::channel::message::embed::Embed;

use super::retrieve_previous;
use crate::{
    core::Context,
    embeds::{EmbedData, MatchCompareMapEmbed, MatchCompareSummaryEmbed},
    pagination::MatchComparePagination,
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "matchcompare", desc = "Compare two multiplayer matches")]
#[bucket(MatchCompare)]
pub struct MatchCompare {
    #[command(desc = "Specify the first match url or match id")]
    match_url_1: String,
    #[command(desc = "Specify the second match url or match id")]
    match_url_2: String,
    #[command(desc = "Specify if the response should be paginated or all at once")]
    output: Option<MatchCompareOutput>,
    #[command(desc = "Specify if it should show comparisons between players or teams")]
    comparison: Option<MatchCompareComparison>,
}

#[derive(CommandOption, CreateOption)]
pub enum MatchCompareOutput {
    #[option(name = "Full", value = "full")]
    Full,
    #[option(name = "Paginated", value = "paginated")]
    Paginated,
}

impl Default for MatchCompareOutput {
    fn default() -> Self {
        Self::Paginated
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum MatchCompareComparison {
    #[option(name = "Compare players", value = "players")]
    Players,
    #[option(name = "Compare teams", value = "teams")]
    Teams,
    #[option(name = "Compare both", value = "both")]
    Both,
}

impl Default for MatchCompareComparison {
    fn default() -> Self {
        Self::Players
    }
}

async fn slash_matchcompare(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = MatchCompare::from_interaction(command.input_data())?;

    matchcompare(ctx, command, args).await
}

async fn matchcompare(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: MatchCompare,
) -> Result<()> {
    let MatchCompare {
        match_url_1,
        match_url_2,
        output,
        comparison,
    } = args;

    let match_id_1 = match matcher::get_osu_match_id(&match_url_1) {
        Some(id) => id,
        None => {
            let content = "Failed to parse `match_url_1`.\n\
                Be sure it's a valid mp url or a match id.";
            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    let match_id_2 = match matcher::get_osu_match_id(&match_url_2) {
        Some(id) => id,
        None => {
            let content = "Failed to parse `match_url_1`.\n\
                Be sure it's a valid mp url or a match id.";
            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    if match_id_1 == match_id_2 {
        let content = "Trying to compare a match with itself huh";
        command.error(&ctx, content).await?;

        return Ok(());
    }

    let match_fut_1 = ctx.osu().osu_match(match_id_1);
    let match_fut_2 = ctx.osu().osu_match(match_id_2);

    let output = output.unwrap_or_default();
    let comparison = comparison.unwrap_or_default();

    let embeds = match tokio::try_join!(match_fut_1, match_fut_2) {
        Ok((mut match_1, mut match_2)) => {
            let previous_fut_1 = retrieve_previous(&mut match_1, ctx.osu());
            let previous_fut_2 = retrieve_previous(&mut match_2, ctx.osu());

            if let Err(err) = tokio::try_join!(previous_fut_1, previous_fut_2) {
                let _ = command.error(&ctx, OSU_API_ISSUE).await;
                let report = Report::new(err)
                    .wrap_err("failed to get history of at least one of the matches");

                return Err(report);
            }

            MatchComparison::new(&mut match_1, &mut match_2).into_embeds(comparison)
        }
        Err(OsuError::NotFound) => {
            let content = "At least one of the two given matches was not found";
            command.error(&ctx, content).await?;

            return Ok(());
        }
        Err(OsuError::Response { status, .. }) if status == 401 => {
            let content =
                "I can't access at least one of the two matches because it was set as private";
            command.error(&ctx, content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = command.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get at least one of the matches");

            return Err(report);
        }
    };

    match output {
        MatchCompareOutput::Full => {
            let mut embeds = embeds.into_iter();

            if let Some(embed) = embeds.next() {
                let builder = MessageBuilder::new().embed(embed);
                command.update(&ctx, &builder).await?;

                let mut interval = interval(Duration::from_secs(1));
                interval.tick().await;

                for embed in embeds {
                    interval.tick().await;
                    command
                        .channel_id
                        .create_message(&ctx, &embed.into(), command.permissions)
                        .await?;
                }
            }
        }
        MatchCompareOutput::Paginated => {
            return MatchComparePagination::builder(embeds)
                .start_by_update()
                .start(ctx, (&mut command).into())
                .await
        }
    }

    Ok(())
}

struct MatchComparison {
    common_maps: Vec<CommonMap>,
    match_1: ProcessedMatch,
    match_2: ProcessedMatch,
    users: HashMap<u32, Username, IntHasher>,
}

impl MatchComparison {
    fn new(match_1: &mut OsuMatch, match_2: &mut OsuMatch) -> Self {
        let users: HashMap<_, _, IntHasher> = match_1
            .users
            .drain()
            .chain(match_2.users.drain())
            .map(|(user_id, user)| (user_id, user.username))
            .collect();

        match_1
            .events
            .retain(|event| matches!(event, MatchEvent::Game { .. }));

        match_2
            .events
            .retain(|event| matches!(event, MatchEvent::Game { .. }));

        let mut processed_1 = ProcessedMatch::new(mem::take(&mut match_1.name), match_1.match_id);
        let mut processed_2 = ProcessedMatch::new(mem::take(&mut match_2.name), match_2.match_id);

        let mut common_maps = Vec::new();

        for mut game_1 in match_1.drain_games() {
            let (map_id, map) = match game_1.map.as_ref().filter(|_| game_1.end_time.is_some()) {
                Some(map) => (map.map_id, map_name(map)),
                None => continue,
            };

            let idx = match_2.events.iter().position(|event| match event {
                MatchEvent::Game { game, .. } => game
                    .map
                    .as_ref()
                    .filter(|_| game.end_time.is_some())
                    .map(|m| m.map_id == map_id)
                    .unwrap_or(false),
                _ => unreachable!(),
            });

            if let Some(idx) = idx {
                let mut game_2 = match match_2.events.remove(idx) {
                    MatchEvent::Game { game, .. } => *game,
                    _ => unreachable!(),
                };

                common_maps.push(CommonMap::new(map, map_id, &mut game_1, &mut game_2));
            } else {
                processed_1.unique_maps.push(UniqueMap::new(map, map_id));
            }
        }

        for game in match_2.drain_games() {
            let (map_id, map) = match game.map.as_ref().filter(|_| game.end_time.is_some()) {
                Some(map) => (map.map_id, map_name(map)),
                None => continue,
            };

            processed_2.unique_maps.push(UniqueMap::new(map, map_id));
        }

        Self {
            common_maps,
            match_1: processed_1,
            match_2: processed_2,
            users,
        }
    }

    fn into_embeds(self, comparison: MatchCompareComparison) -> Vec<Embed> {
        let mut embeds = Vec::with_capacity(self.common_maps.len() + 2);
        let common_total = self.common_maps.len();

        let maps_total = self.common_maps.len()
            + self.match_1.unique_maps.len()
            + self.match_2.unique_maps.len();

        let tuple = (common_total + 1, common_total, maps_total);
        let summary_1 = MatchCompareSummaryEmbed::new(&self.common_maps, &self.match_1, tuple);

        let tuple = (common_total + 2, common_total, maps_total);
        let summary_2 = MatchCompareSummaryEmbed::new(&self.common_maps, &self.match_2, tuple);

        let iter = self
            .common_maps
            .into_iter()
            .zip(1..)
            .map(|(map, i)| {
                MatchCompareMapEmbed::new(
                    map,
                    &self.match_1.name,
                    &self.match_2.name,
                    &self.users,
                    comparison,
                    (i, common_total, maps_total),
                )
            })
            .map(EmbedData::build);

        embeds.extend(iter);

        embeds.push(summary_1.build());
        embeds.push(summary_2.build());

        embeds
    }
}

pub struct CommonMap {
    pub map: String,
    pub map_id: u32,
    pub match_1: Vec<MatchCompareScore>,
    pub match_1_scores: [u32; 3],
    pub match_2: Vec<MatchCompareScore>,
    pub match_2_scores: [u32; 3],
}

trait HasScore {
    fn team(&self) -> Team;
    fn score(&self) -> u32;
}

impl HasScore for MatchScore {
    fn team(&self) -> Team {
        self.team
    }

    fn score(&self) -> u32 {
        self.score
    }
}

impl CommonMap {
    fn new(map: String, map_id: u32, game_1: &mut MatchGame, game_2: &mut MatchGame) -> Self {
        let mut match_1_scores = [0; 3];

        let mut match_1: Vec<_> = game_1
            .scores
            .drain(..)
            .map(|score| MatchCompareScore::new(score, game_1.mode))
            .inspect(|score| match_1_scores[score.team as usize] += score.score)
            .collect();

        let mut match_2_scores = [0; 3];

        let mut match_2: Vec<_> = game_2
            .scores
            .drain(..)
            .map(|score| MatchCompareScore::new(score, game_2.mode))
            .inspect(|score| match_2_scores[score.team as usize] += score.score)
            .collect();

        let score_compare = |a: &MatchCompareScore, b: &MatchCompareScore| {
            (a.team as u8)
                .cmp(&(b.team as u8))
                .then_with(|| b.score.cmp(&a.score))
        };

        match_1.sort_unstable_by(score_compare);
        match_2.sort_unstable_by(score_compare);

        Self {
            map,
            map_id,
            match_1,
            match_1_scores,
            match_2,
            match_2_scores,
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
        // TODO: make this prettier
        let mods = score.mods.clone().with_mode(mode).expect("invalid mods");

        Self {
            grade: calculate_grade(mode, &mods, &score.statistics),
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
    pub name: String,
    pub match_id: u32,
    pub unique_maps: Vec<UniqueMap>,
}

impl ProcessedMatch {
    fn new(name: String, match_id: u32) -> Self {
        let name = match name.cow_escape_markdown() {
            Cow::Borrowed(_) => name,
            Cow::Owned(owned) => owned,
        };

        Self {
            name,
            match_id,
            unique_maps: Vec::new(),
        }
    }
}

pub struct UniqueMap {
    pub map: String,
    pub map_id: u32,
}

impl UniqueMap {
    fn new(map: String, map_id: u32) -> Self {
        Self { map, map_id }
    }
}

fn map_name(map: &BeatmapCompact) -> String {
    let mut name = String::new();

    if let Some(ref mapset) = map.mapset {
        let _ = write!(name, "{}", mapset.title.cow_escape_markdown());
    } else {
        name.push_str("<unknown title>")
    }

    let _ = write!(name, " [{}]", map.version.cow_escape_markdown());

    name
}
