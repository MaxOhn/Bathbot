use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::Write,
    mem,
    sync::Arc,
};

use bathbot_macros::{command, SlashCommand};
use bathbot_util::{constants::OSU_API_ISSUE, matcher, IntHasher, MessageBuilder};
use eyre::{Report, Result};
use rosu_v2::prelude::{
    GameModIntermode, MatchGame, Osu, OsuError, OsuMatch, OsuResult, Team, TeamType, UserCompact,
};
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, MatchCostEmbed},
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "matchcost",
    desc = "Display performance ratings for a multiplayer match",
    help = "Calculate a performance rating for each player in the given multiplayer match.\n\
    Here's the current [formula](https://i.imgur.com/7KFwcUS.png).\n\
    Additionally, scores with the EZ mod are multiplied by 1.7 beforehand.\n\n\
    Keep in mind that all bots use different formulas \
    so comparing with values from other bots makes no sense."
)]
pub struct MatchCost<'a> {
    #[command(desc = "Specify a match url or match id")]
    match_url: Cow<'a, str>,
    #[command(
        min_value = 0,
        desc = "Specify the amount of warmups to ignore (defaults to 2)",
        help = "Since warmup maps commonly want to be skipped for performance calculations, \
        this option allows you to specify how many maps should be ignored in the beginning.\n\
        If no value is specified, it defaults to 2."
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

        let warmups = args
            .num
            .or_else(|| args.next().and_then(|num| num.parse().ok()))
            .map(|n| n as usize);

        Ok(Self {
            match_url,
            warmups,
            skip_last: None,
            ez_mult: None,
        })
    }
}

#[command]
#[desc("Display performance ratings for a multiplayer match")]
#[help(
    "Calculate a performance rating for each player \
     in the given multiplayer match.\nThe optional second \
     argument is the amount of played warmups, defaults to 2.\n\
     Here's the current [formula](https://i.imgur.com/7KFwcUS.png).\n\
     Keep in mind that all bots use different formulas so comparing \
     with values from other bots makes no sense."
)]
#[usage("[match url / match id] [amount of warmups]")]
#[examples("58320988 1", "https://osu.ppy.sh/community/matches/58320988")]
#[aliases("mc", "matchcost")]
#[group(AllModes)]
async fn prefix_matchcosts(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match MatchCost::args(args) {
        Ok(args) => matchcosts(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_matchcost(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = MatchCost::from_interaction(command.input_data())?;

    matchcosts(ctx, (&mut command).into(), args).await
}

const USER_LIMIT: usize = 50;
const TOO_MANY_PLAYERS_TEXT: &str = "Too many players, cannot display message :(";

async fn matchcosts(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: MatchCost<'_>) -> Result<()> {
    let MatchCost {
        match_url,
        warmups,
        skip_last,
        ez_mult,
    } = args;

    let match_id = match matcher::get_osu_match_id(&match_url) {
        Some(id) => id,
        None => {
            let content = "Failed to parse match url.\n\
                Be sure it's a valid mp url or a match id.";

            return orig.error(&ctx, content).await;
        }
    };

    let warmups = warmups.unwrap_or(2);
    let ez_mult = ez_mult.unwrap_or(1.0);
    let skip_last = skip_last.unwrap_or(0);

    // Retrieve the match
    let (mut osu_match, games) = match ctx.osu().osu_match(match_id).await {
        Ok(mut osu_match) => {
            retrieve_previous(&mut osu_match, ctx.osu()).await?;

            let games_iter = osu_match
                .drain_games()
                .filter(|game| game.end_time.is_some())
                .skip(warmups);

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

            return orig.error(&ctx, content).await;
        }
        Err(OsuError::Response { status, .. }) if status == 401 => {
            let content = "I can't access the match because it was set as private";

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get match");

            return Err(report);
        }
    };

    // Count different users
    let users: HashSet<_> = games
        .iter()
        .flat_map(|game| game.scores.iter())
        .filter(|s| s.score > 0)
        .map(|s| s.user_id)
        .collect();

    // Prematurely abort if its too many players to display in a message
    if users.len() > USER_LIMIT {
        return orig.error(&ctx, TOO_MANY_PLAYERS_TEXT).await;
    }

    // Process match
    let (description, match_result) = if games.is_empty() {
        let mut description = format!("No games played yet beyond the {warmups} warmup");

        if warmups != 1 {
            description.push('s');
        }

        (Some(description), None)
    } else {
        let result = process_match(&games, osu_match.end_time.is_some(), &osu_match.users);

        (None, Some(result))
    };

    // Accumulate all necessary data
    // TODO: pagination(?)
    let embed_data = match MatchCostEmbed::new(&mut osu_match, description, match_result) {
        Some(data) => data,
        None => return orig.error(&ctx, TOO_MANY_PLAYERS_TEXT).await,
    };

    let embed = embed_data.build();

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

    // Creating the embed
    let mut builder = MessageBuilder::new().embed(embed);

    if !content.is_empty() {
        builder = builder.content(content);
    }

    orig.create_message(&ctx, &builder).await?;

    Ok(())
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

macro_rules! sort {
    ($slice:expr) => {
        $slice.sort_unstable_by(|(.., a), (.., b)| b.partial_cmp(a).unwrap_or(Ordering::Equal));
    };
}

// flat additive bonus for each participated game
const FLAT_PARTICIPATION_BONUS: f32 = 0.5;

// exponent base, the higher - the higher is the difference
// between players who played a lot and players who played fewer
const BASE_PARTICIPATION_BONUS: f32 = 1.4;

// exponent, low: logithmically ~ high: linear
const EXP_PARTICIPATION_BONUS: f32 = 0.6;

// instead of considering tb score once, consider it this many times
const TIEBREAKER_BONUS: f32 = 2.0;

// global multiplier per combination (if at least 3)
const MOD_BONUS: f32 = 0.02;

pub fn process_match(
    games: &[MatchGame],
    finished: bool,
    users: &HashMap<u32, UserCompact>,
) -> MatchResult {
    let mut teams = HashMap::with_hasher(IntHasher);
    let mut point_costs = HashMap::with_hasher(IntHasher);
    let mut mods = HashMap::with_hasher(IntHasher);
    let team_vs = games[0].team_type == TeamType::TeamVS;
    let mut match_scores = MatchScores(0, 0);

    // Calculate point scores for each score in each game
    for game in games.iter() {
        let score_sum: f32 = game.scores.iter().map(|s| s.score as f32).sum();

        let avg = score_sum / game.scores.iter().filter(|s| s.score > 0).count() as f32;
        let mut team_scores = HashMap::with_capacity(team_vs as usize + 1);

        for score in game.scores.iter().filter(|s| s.score > 0) {
            mods.entry(score.user_id)
                .or_insert_with(HashSet::new)
                .insert(score.mods.clone() - GameModIntermode::NoFail);

            let mut point_cost = score.score as f32 / avg;

            point_cost += FLAT_PARTICIPATION_BONUS;

            point_costs
                .entry(score.user_id)
                .or_insert_with(Vec::new)
                .push(point_cost);

            teams.entry(score.user_id).or_insert(score.team);

            team_scores
                .entry(score.team)
                .and_modify(|e| *e += score.score)
                .or_insert(score.score);
        }

        let (winner_team, _) = team_scores
            .into_iter()
            .max_by_key(|(_, score)| *score)
            .unwrap_or((Team::None, 0));

        match_scores.incr(winner_team);
    }

    // Tiebreaker bonus
    if let Some(game) = games
        .last()
        .filter(|_| finished && games.len() > 4 && match_scores.difference() == 1)
    {
        point_costs
            .iter_mut()
            .filter(|(&user_id, _)| game.scores.iter().any(|score| score.user_id == user_id))
            .filter_map(|(_, costs)| costs.last_mut())
            .for_each(|value| {
                *value -= FLAT_PARTICIPATION_BONUS;
                *value *= TIEBREAKER_BONUS;
                *value += FLAT_PARTICIPATION_BONUS;
            });
    }

    // Mod combinations bonus
    let mods_count = mods
        .into_iter()
        .filter(|(_, mods)| mods.len() > 2)
        .map(|(id, mods)| (id, mods.len() - 2));

    for (user_id, count) in mods_count {
        let multiplier = 1.0 + count as f32 * MOD_BONUS;

        point_costs.entry(user_id).and_modify(|point_scores| {
            point_scores
                .iter_mut()
                .for_each(|point_score| *point_score *= multiplier);
        });
    }

    // Calculate match costs by combining point costs
    let mut data = HashMap::with_capacity(team_vs as usize + 1);
    let mut highest_cost = 0.0;
    let mut mvp_avatar_url = None;

    for (user_id, point_costs) in point_costs {
        let sum: f32 = point_costs.iter().sum();
        let costs_len = point_costs.len() as f32;
        let mut match_cost = sum / costs_len;

        let exp = match games.len() {
            1 => 0.0,
            len => (costs_len - 1.0) / (len as f32 - 1.0),
        };

        match_cost *= BASE_PARTICIPATION_BONUS.powf(exp.powf(EXP_PARTICIPATION_BONUS));

        data.entry(*teams.get(&user_id).unwrap())
            .or_insert_with(Vec::new)
            .push((user_id, match_cost));

        if match_cost > highest_cost {
            highest_cost = match_cost;

            if let Some(user) = users.get(&user_id) {
                mvp_avatar_url.replace(user.avatar_url.as_str());
            }
        }
    }

    let mvp_avatar_url = mvp_avatar_url.map_or_else(String::new, |url| url.to_owned());

    if team_vs {
        let blue = match data.remove(&Team::Blue) {
            Some(mut team) => {
                sort!(team);

                team
            }
            None => Vec::new(),
        };

        let red = match data.remove(&Team::Red) {
            Some(mut team) => {
                sort!(team);

                team
            }
            None => Vec::new(),
        };

        MatchResult::team(mvp_avatar_url, match_scores, blue, red)
    } else {
        let mut players = data.remove(&Team::None).unwrap_or_default();
        sort!(players);

        MatchResult::solo(mvp_avatar_url, players)
    }
}

type PlayerResult = (u32, f32);
type TeamResult = Vec<PlayerResult>;

pub enum MatchResult {
    TeamVS {
        blue: TeamResult,
        red: TeamResult,
        mvp_avatar_url: String,
        match_scores: MatchScores,
    },
    HeadToHead {
        players: TeamResult,
        mvp_avatar_url: String,
    },
}

impl MatchResult {
    fn team(
        mvp_avatar_url: String,
        match_scores: MatchScores,
        blue: TeamResult,
        red: TeamResult,
    ) -> Self {
        Self::TeamVS {
            mvp_avatar_url,
            match_scores,
            blue,
            red,
        }
    }

    fn solo(mvp_avatar_url: String, players: TeamResult) -> Self {
        Self::HeadToHead {
            mvp_avatar_url,
            players,
        }
    }
}

#[derive(Copy, Clone)]
pub struct MatchScores(u8, u8);

impl MatchScores {
    fn incr(&mut self, team: Team) {
        match team {
            Team::Blue => self.0 = self.0.saturating_add(1),
            Team::Red => self.1 = self.1.saturating_add(1),
            Team::None => {}
        }
    }

    pub fn blue(self) -> u8 {
        self.0
    }

    pub fn red(self) -> u8 {
        self.1
    }

    fn difference(&self) -> u8 {
        let min = self.0.min(self.1);
        let max = self.0.max(self.1);

        max - min
    }
}
