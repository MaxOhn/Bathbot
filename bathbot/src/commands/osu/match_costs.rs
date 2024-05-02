use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Write,
    mem,
};

use bathbot_macros::{command, SlashCommand};
use bathbot_util::{constants::OSU_API_ISSUE, matcher, IntHasher};
use eyre::{Report, Result};
use rosu_v2::{
    model::mods::GameModsIntermode,
    prelude::{
        GameModIntermode, MatchGame, Osu, OsuError, OsuMatch, OsuResult, Team, TeamType, User,
    },
};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};

use crate::{
    active::{impls::MatchCostPagination, ActiveMessages},
    core::commands::{
        prefix::{Args, ArgsNum},
        CommandOrigin,
    },
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "matchcost",
    desc = "Display performance ratings for a multiplayer match",
    help = "Calculate a performance rating for each player in the given multiplayer match.\n\
    Current formula: <https://i.imgur.com/zuii7Oj.png> ([desmos](https://www.desmos.com/calculator/mm4tins990))"
)]
pub struct MatchCost<'a> {
    #[command(desc = "Specify a match url or match id")]
    match_url: Cow<'a, str>,
    #[command(
        min_value = 0,
        desc = "Specify the amount of warmups to ignore (defaults to 0)",
        help = "Since warmup maps commonly want to be skipped for performance calculations, \
        this option allows you to specify how many maps should be ignored in the beginning.\n\
        If no value is specified, it defaults to 0."
    )]
    warmups: Option<usize>,
    #[command(
        max_value = 100.0,
        desc = "Specify a multiplier for EZ scores",
        help = "Specify a multiplier for EZ scores.\n\
        The suggested multiplier range is 1.0-2.0"
    )]
    ez_mult: Option<f32>,
    #[command(
        min_value = 0,
        desc = "Specify the amount of maps to ignore at the end (defaults to 0)",
        help = "In case the last few maps were just for fun, \
        this options allows to ignore them for the performance rating.\n\
        Alternatively, in combination with the `warmups` option, \
        you can check the rating for specific maps.\n\
        If no value is specified, it defaults to 0."
    )]
    skip_last: Option<usize>,
    #[command(desc = "How the data should be displayed")]
    display: Option<MatchCostDisplay>,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Default)]
pub enum MatchCostDisplay {
    #[default]
    #[option(name = "Compact", value = "compact")]
    Compact,
    #[option(name = "Full", value = "full")]
    Full,
}

impl<'m> MatchCost<'m> {
    fn args(mut args: Args<'m>) -> Result<Self, &'static str> {
        let match_url = match args.next() {
            Some(arg) => arg.into(),
            None => {
                let content = "The first argument must be either a match \
                    id or the multiplayer link to a match";

                return Err(content);
            }
        };

        let warmups = match args.num {
            ArgsNum::Value(n) => Some(n as usize),
            ArgsNum::Random | ArgsNum::None => args.next().and_then(|arg| arg.parse().ok()),
        };

        Ok(Self {
            match_url,
            warmups,
            skip_last: None,
            ez_mult: None,
            display: None,
        })
    }
}

#[command]
#[desc("Display performance ratings for a multiplayer match")]
#[help(
    "Calculate a performance rating for each player \
     in the given multiplayer match.\nThe optional second \
     argument is the amount of played warmups, defaults to 0.\n\
     Current formula: <https://i.imgur.com/zuii7Oj.png> ([desmos](https://www.desmos.com/calculator/mm4tins990))"
)]
#[usage("[match url / match id] [amount of warmups]")]
#[examples("58320988 1", "https://osu.ppy.sh/community/matches/58320988")]
#[aliases("mc", "matchcost")]
#[group(AllModes)]
async fn prefix_matchcosts(msg: &Message, args: Args<'_>) -> Result<()> {
    match MatchCost::args(args) {
        Ok(args) => matchcosts(msg.into(), args).await,
        Err(content) => {
            msg.error(content).await?;

            Ok(())
        }
    }
}

async fn slash_matchcost(mut command: InteractionCommand) -> Result<()> {
    let args = MatchCost::from_interaction(command.input_data())?;

    matchcosts((&mut command).into(), args).await
}

async fn matchcosts(orig: CommandOrigin<'_>, args: MatchCost<'_>) -> Result<()> {
    let owner = orig.user_id()?;

    let MatchCost {
        match_url,
        warmups,
        skip_last,
        ez_mult,
        display,
    } = args;

    let Some(match_id) = matcher::get_osu_match_id(&match_url) else {
        let content = "Failed to parse match url.\n\
            Be sure it's a valid mp url or a match id.";

        return orig.error(content).await;
    };

    let warmups = warmups.unwrap_or(0);
    let ez_mult = ez_mult.unwrap_or(1.0);
    let skip_last = skip_last.unwrap_or(0);
    let osu = Context::osu();

    // Retrieve the match
    let (osu_match, games) = match osu.osu_match(match_id).await {
        Ok(mut osu_match) => {
            retrieve_previous(&mut osu_match, osu).await?;

            let games_iter = osu_match
                .drain_games()
                .filter(|game| game.end_time.is_some())
                .skip(warmups)
                .map(|mut game| {
                    game.scores.retain(|score| score.score > 0);

                    game
                });

            let mut games: Vec<_> = if ez_mult != 1.0 {
                games_iter
                    .map(|mut game| {
                        game.scores.iter_mut().for_each(|score| {
                            if score.mods.contains(GameModIntermode::Easy) {
                                score.score = (score.score as f32 * ez_mult) as u32;
                            }
                        });

                        game
                    })
                    .collect()
            } else {
                games_iter.collect()
            };

            if skip_last > 0 {
                games.truncate(games.len() - skip_last);
            }

            (osu_match, games)
        }
        Err(OsuError::NotFound) => {
            let content = format!("No match with id `{match_id}` was found");

            return orig.error(content).await;
        }
        Err(OsuError::Response { status, .. }) if status == 401 => {
            let content = "I can't access the match because it was set as private";

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get match");

            return Err(err);
        }
    };

    let match_result = if games.is_empty() {
        let mut description = format!("No games played yet beyond the {warmups} warmup");

        if warmups != 1 {
            description.push('s');
        }

        MatchResult::NoGames { description }
    } else {
        process_match(&games, osu_match.end_time.is_some(), &osu_match.users)
    };

    let mut content = String::new();

    if warmups > 0 {
        content.push_str("Ignoring the first ");

        if warmups == 1 {
            content.push_str("map");
        } else {
            let _ = write!(content, "{warmups} maps");
        }

        content.push_str(" as warmup");
    }

    if ez_mult != 1.0 {
        let _ = if content.is_empty() {
            write!(content, "EZ multiplier: {ez_mult:.2}")
        } else {
            write!(content, " (EZ multiplier: {ez_mult:.2}):")
        };
    } else if !content.is_empty() {
        content.push(':');
    }

    let pagination = MatchCostPagination::builder()
        .osu_match(osu_match)
        .content(content.into_boxed_str())
        .display(display.unwrap_or_default())
        .msg_owner(owner)
        .result(match_result)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

pub async fn retrieve_previous(osu_match: &mut OsuMatch, osu: &Osu) -> OsuResult<()> {
    let mut curr = &*osu_match;
    let mut prev: Option<OsuMatch> = None;

    // Retrieve at most 500 previous events
    for _ in 0..5 {
        match curr.get_previous(osu).await {
            Some(Ok(next_prev)) => {
                let prev_opt = prev.take();
                curr = &*prev.get_or_insert(next_prev);

                if let Some(mut prev) = prev_opt {
                    prev.events.append(&mut osu_match.events);
                    mem::swap(&mut prev.events, &mut osu_match.events);
                    osu_match.users.extend(prev.users);
                }
            }
            Some(Err(err)) => return Err(err),
            None => break,
        }
    }

    if let Some(mut prev) = prev {
        prev.events.append(&mut osu_match.events);
        mem::swap(&mut prev.events, &mut osu_match.events);
        osu_match.users.extend(prev.users);
    }

    Ok(())
}

// flat additive performance cost bonus for each player
const FLAT_BONUS: f32 = 0.5;

// exponent base; maximum participation bonus for playing each game
const BASE_PARTICIPATION_BONUS: f32 = 1.5;

// exponent; curve to reach the maximum participation bonus
// <0.85: fast up then slow down; >0.85: slow up then speed up
const EXP_PARTICIPATION_BONUS: f32 = 0.6;

// multiplier bonus per combination (if at least 3)
const MOD_BONUS: f32 = 0.02;

// performing average on the tiebreaker will reward this flat amount
const TIEBREAKER_FACTOR: f32 = 0.25;

// any tiebreaker performance cost >=2 gets the same bonus
const MAX_TIEBREAKER_BONUS: f32 = 0.5;

pub fn process_match(
    games: &[MatchGame],
    finished: bool,
    users: &HashMap<u32, User>,
) -> MatchResult {
    let mut users_mods = UsersMods::default();
    let mut users_performance_costs = UsersPerformanceCosts::default();
    let mut users_team = UsersTeam::default();
    let mut teams_win_count = TeamsWinCount::default();

    for game in games.iter() {
        let score_sum = game.scores.iter().fold(0, |sum, score| sum + score.score);
        let score_count = game.scores.len();
        let score_avg = score_sum as f32 / score_count as f32;

        let mut teams_score = TeamsScore::default();

        for score in game.scores.iter() {
            users_mods.update(score.user_id, score.mods.clone());
            users_performance_costs.update(score.user_id, score.score, score_avg);
            users_team.update(score.user_id, score.team);
            teams_score.update(score.team, score.score);
        }

        teams_win_count.add_win(teams_score.winner());
    }

    let tiebreaker_game = games
        .last()
        .filter(|_| finished && games.len() > 4 && teams_win_count.diff() == 1);

    let match_costs =
        users_performance_costs.match_costs(games.len(), &users_mods, tiebreaker_game);

    let mvp_avatar_url = match_costs
        .iter()
        .reduce(|(mvp_user_id, mvp_entry), (user_id, entry)| {
            if entry.match_cost() > mvp_entry.match_cost() {
                (user_id, entry)
            } else {
                (mvp_user_id, mvp_entry)
            }
        })
        .and_then(|(user_id, _)| users.get(user_id))
        .map_or_else(Box::default, |user| Box::from(user.avatar_url.as_str()));

    if games[0].team_type == TeamType::TeamVS {
        let mut blue = TeamResult::new(teams_win_count.get(Team::Blue));
        let mut red = TeamResult::new(teams_win_count.get(Team::Red));

        for (&user_id, entry) in match_costs.iter() {
            let Some(team @ (Team::Blue | Team::Red)) = users_team.get(user_id) else {
                continue;
            };

            let entry = UserMatchCostEntry {
                user_id,
                performance_cost: entry.performance_cost,
                participation_bonus_factor: entry.participation_bonus_factor,
                mods_bonus_factor: entry.mods_bonus_factor,
                tiebreaker_bonus: entry.tiebreaker_bonus,
                match_cost: entry.match_cost(),
                avg_score: entry.avg_score,
            };

            match team {
                Team::Blue => blue.players.push(entry),
                Team::Red => red.players.push(entry),
                Team::None => unreachable!(),
            }
        }

        UserMatchCostEntry::sort(&mut blue.players);
        UserMatchCostEntry::sort(&mut red.players);

        MatchResult::TeamVS {
            blue,
            red,
            mvp_avatar_url,
        }
    } else {
        let mut players: Vec<_> = match_costs
            .iter()
            .map(|(user_id, entry)| UserMatchCostEntry {
                user_id: *user_id,
                performance_cost: entry.performance_cost,
                participation_bonus_factor: entry.participation_bonus_factor,
                mods_bonus_factor: entry.mods_bonus_factor,
                tiebreaker_bonus: entry.tiebreaker_bonus,
                match_cost: entry.match_cost(),
                avg_score: entry.avg_score,
            })
            .collect();

        UserMatchCostEntry::sort(&mut players);

        MatchResult::HeadToHead {
            players,
            mvp_avatar_url,
        }
    }
}

/// Keeps track of all mod combinations a user has played
#[derive(Default)]
struct UsersMods {
    entries: HashMap<u32, HashSet<GameModsIntermode>, IntHasher>,
}

impl UsersMods {
    fn update(&mut self, user_id: u32, mods: GameModsIntermode) {
        self.entries
            .entry(user_id)
            .or_default()
            .insert(mods - GameModIntermode::NoFail);
    }

    fn get_count(&self, user_id: u32) -> Option<usize> {
        self.entries.get(&user_id).map(HashSet::len)
    }
}

/// For each user, store the performance cost of all their scores
#[derive(Default)]
struct UsersPerformanceCosts {
    entries: HashMap<u32, Vec<PerformanceCost>, IntHasher>,
}

impl UsersPerformanceCosts {
    fn update(&mut self, user_id: u32, score: u32, score_avg: f32) {
        let performance_cost = PerformanceCost {
            score,
            performance_cost: score as f32 / score_avg,
        };

        self.entries
            .entry(user_id)
            .or_default()
            .push(performance_cost);
    }

    fn match_costs(
        &self,
        games_count: usize,
        users_mods: &UsersMods,
        tiebreaker_game: Option<&MatchGame>,
    ) -> HashMap<u32, MatchCostEntry, IntHasher> {
        let mut match_costs = HashMap::with_capacity_and_hasher(self.entries.len(), IntHasher);

        for (user_id, entries) in self.entries.iter() {
            let (performance_cost_sum, score_sum) =
                entries
                    .iter()
                    .fold((0.0, 0), |(performance_cost_sum, score_sum), entry| {
                        (
                            performance_cost_sum + entry.performance_cost,
                            score_sum + entry.score,
                        )
                    });

            let scores_len = entries.len() as f32;
            let avg_score = (score_sum as f32 / scores_len) as u32;
            let performance_cost = performance_cost_sum / scores_len + FLAT_BONUS;

            let mut tiebreaker_bonus = 0.0;

            if let Some(game) = tiebreaker_game {
                if game.scores.iter().any(|score| score.user_id == *user_id) {
                    if let Some(entry) = entries.last() {
                        tiebreaker_bonus =
                            MAX_TIEBREAKER_BONUS.min(TIEBREAKER_FACTOR * entry.performance_cost);
                    }
                }
            }

            let exp = if games_count <= 1 {
                0.0
            } else {
                (scores_len - 1.0) / (games_count - 1) as f32
            };

            let participation_bonus_factor =
                BASE_PARTICIPATION_BONUS.powf(exp.powf(EXP_PARTICIPATION_BONUS));

            let mods_used = users_mods.get_count(*user_id).unwrap_or(0) as u32;

            let mut mods_bonus_factor = 1.0;

            if mods_used > 2 {
                mods_bonus_factor += MOD_BONUS * (mods_used - 2) as f32;
            }

            let entry = MatchCostEntry {
                performance_cost,
                participation_bonus_factor,
                mods_bonus_factor,
                tiebreaker_bonus,
                avg_score,
            };

            match_costs.insert(*user_id, entry);
        }

        match_costs
    }
}

struct PerformanceCost {
    score: u32,
    performance_cost: f32,
}

/// Store each user's team.
///
/// If a user has played in multiple teams, only the first one is stored.
#[derive(Default)]
struct UsersTeam {
    entries: HashMap<u32, Team, IntHasher>,
}

impl UsersTeam {
    fn update(&mut self, user_id: u32, team: Team) {
        self.entries.entry(user_id).or_insert(team);
    }

    fn get(&self, user_id: u32) -> Option<Team> {
        self.entries.get(&user_id).copied()
    }
}

/// The score sum of all users in a team.
#[derive(Default)]
struct TeamsScore {
    entries: HashMap<Team, u32, IntHasher>,
}

impl TeamsScore {
    fn update(&mut self, team: Team, score: u32) {
        *self.entries.entry(team).or_default() += score;
    }

    fn winner(&self) -> Team {
        self.entries
            .iter()
            .max_by_key(|(_, score)| **score)
            .map_or(Team::None, |(team, _)| *team)
    }
}

/// The amount of games that a team won because it had more total score.
#[derive(Default)]
struct TeamsWinCount {
    entries: HashMap<Team, u32, IntHasher>,
}

impl TeamsWinCount {
    fn add_win(&mut self, team: Team) {
        *self.entries.entry(team).or_default() += 1;
    }

    fn diff(&self) -> u32 {
        if let (Some(blue), Some(red)) =
            (self.entries.get(&Team::Blue), self.entries.get(&Team::Red))
        {
            blue.abs_diff(*red)
        } else {
            0
        }
    }

    fn get(&self, team: Team) -> u32 {
        self.entries.get(&team).copied().unwrap_or(0)
    }
}

struct MatchCostEntry {
    performance_cost: f32,
    participation_bonus_factor: f32,
    mods_bonus_factor: f32,
    tiebreaker_bonus: f32,
    avg_score: u32,
}

impl MatchCostEntry {
    fn match_cost(&self) -> f32 {
        (self.performance_cost * self.participation_bonus_factor * self.mods_bonus_factor)
            + self.tiebreaker_bonus
    }
}

pub struct UserMatchCostEntry {
    pub user_id: u32,
    pub performance_cost: f32,
    pub participation_bonus_factor: f32,
    pub mods_bonus_factor: f32,
    pub tiebreaker_bonus: f32,
    pub match_cost: f32,
    pub avg_score: u32,
}

impl UserMatchCostEntry {
    fn sort(entries: &mut [Self]) {
        entries.sort_unstable_by(|a, b| b.match_cost.total_cmp(&a.match_cost));
    }
}

pub struct TeamResult {
    pub players: Vec<UserMatchCostEntry>,
    pub win_count: u32,
}

impl TeamResult {
    pub fn new(win_count: u32) -> Self {
        Self {
            players: Vec::new(),
            win_count,
        }
    }
}

pub enum MatchResult {
    TeamVS {
        blue: TeamResult,
        red: TeamResult,
        mvp_avatar_url: Box<str>,
    },
    HeadToHead {
        players: Vec<UserMatchCostEntry>,
        mvp_avatar_url: Box<str>,
    },
    NoGames {
        description: String,
    },
}
