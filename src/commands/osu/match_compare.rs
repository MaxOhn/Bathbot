use std::{fmt::Write, mem, sync::Arc, time::Duration};

use command_macros::SlashCommand;
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::{
    BeatmapCompact, GameMode, GameMods, Grade, MatchEvent, MatchGame, MatchScore, OsuError,
    OsuMatch, Team, Username,
};
use tokio::time::interval;
use twilight_interactions::command::{CommandOption, CommandOption, CreateCommand, CreateOption};
use twilight_model::{
    application::{
        command::CommandOptionChoice,
        interaction::{application_command::CommandOptionValue, ApplicationCommand},
    },
    id::Id,
};

use crate::{
    core::Context,
    embeds::{EmbedBuilder, EmbedData, MatchCompareMapEmbed, MatchCompareSummaryEmbed},
    error::Error,
    pagination::{MatchComparePagination, Pagination},
    util::{constants::OSU_API_ISSUE, matcher, ScoreExt},
    BotResult,
};

use super::retrieve_previous;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "matchcompare")]
/// Compare two multiplayer matches
pub struct MatchCompare {
    /// Specify the first match url or match id
    match_url_1: String,
    /// Specify the second match url or match id
    match_url_2: String,
    /// Specify if the response should be paginated or all at once
    output: Option<MatchCompareOutput>,
    /// Specify if it should show comparisons between players or teams
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

#[derive(CommandOption, CreateOption)]
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

async fn slash_matchcompare(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let args = MatchCompare::from_interaction(command.input_data())?;

    matchcompare(ctx, command, args).await
}

async fn matchcompare(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    args: MatchCompare,
) -> BotResult<()> {
    let MatchCompare {
        match_id_1,
        match_id_2,
        output,
        comparison,
    } = args;

    let match_id_1 = match matcher::get_osu_match_id(&match_id_1) {
        Some(id) => id,
        None => {
            let content = "Failed to parse `match_url_1`.\n\
                Be sure it's a valid mp url or a match id.";

            return command.error(&ctx, content).await;
        }
    };

    let match_id_2 = match matcher::get_osu_match_id(&match_id_2) {
        Some(id) => id,
        None => {
            let content = "Failed to parse `match_url_1`.\n\
                Be sure it's a valid mp url or a match id.";

            return command.error(&ctx, content).await;
        }
    };

    if match_id_1 == match_id_2 {
        let content = "Trying to compare a match with itself huh";

        return command.error(&ctx, content).await;
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

                return Err(err.into());
            }

            MatchComparison::new(&mut match_1, &mut match_2).into_embeds(comparison)
        }
        Err(OsuError::NotFound) => {
            let content = "At least one of the two given matches was not found";

            return command.error(&ctx, content).await;
        }
        Err(OsuError::Response { status, .. }) if status == 401 => {
            let content =
                "I can't access at least one of the two matches because it was set as private";

            return command.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = command.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    match output {
        MatchCompareOutput::Full => {
            let mut embeds = embeds.into_iter();

            if let Some(embed) = embeds.next() {
                let builder = MessageBuilder::new().embed(embed.build());
                command.create_message(&ctx, &builder).await?;

                let mut interval = interval(Duration::from_secs(1));
                interval.tick().await;
                let channel = (Id::new(1), command.channel_id);

                for embed in embeds {
                    interval.tick().await;
                    let embed = embed.build();
                    channel.create_message(&ctx, embed.into()).await?;
                }
            }
        }
        MatchCompareOutput::Paginated => {
            if let Some(embed) = embeds.first().cloned().map(EmbedBuilder::build) {
                let builder = MessageBuilder::new().embed(embed);
                let response_raw = command.create_message(&ctx, &builder).await?;
                let response = response_raw.model().await?;
                let pagination = MatchComparePagination::new(response, embeds);
                let owner = command.user_id()?;

                tokio::spawn(async move {
                    if let Err(err) = pagination.start(&ctx, owner, 60).await {
                        warn!("{:?}", Report::new(err));
                    }
                });
            }
        }
    }

    Ok(())
}

struct MatchComparison {
    common_maps: Vec<CommonMap>,
    match_1: ProcessedMatch,
    match_2: ProcessedMatch,
    users: HashMap<u32, Username>,
}

impl MatchComparison {
    fn new(match_1: &mut OsuMatch, match_2: &mut OsuMatch) -> Self {
        let users: HashMap<_, _> = match_1
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

    fn into_embeds(self, comparison: MatchCompareComparison) -> Vec<EmbedBuilder> {
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
            .map(EmbedData::into_builder);

        embeds.extend(iter);

        embeds.push(summary_1.into_builder());
        embeds.push(summary_2.into_builder());

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
    pub mods: GameMods,
    pub acc: f32,
    pub combo: u32,
    pub score: u32,
    pub team: Team,
}

impl MatchCompareScore {
    fn new(score: MatchScore, mode: GameMode) -> Self {
        Self {
            grade: score.grade(mode),
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
        let _ = write!(name, "{}", mapset.title);
    } else {
        name.push_str("<unknown title>")
    }

    let _ = write!(name, " [{}]", map.version);

    name
}
