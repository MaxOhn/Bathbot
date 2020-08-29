use crate::{
    arguments::{Args, MatchArgs},
    embeds::{EmbedData, MatchCostEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use futures::future::{try_join_all, TryFutureExt};
use itertools::Itertools;
use rosu::{
    backend::requests::{MatchRequest, UserRequest},
    models::{GameMods, Match, Team, TeamType},
};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::Write,
    sync::Arc,
};
use twilight::model::channel::Message;

#[command]
#[short_desc("Display performance ratings for a multiplayer match")]
#[long_desc(
    "Calculate a performance rating for each player \
     in the given multiplayer match. The optional second \
     argument is the amount of played warmups, defaults to 2.\n\
     Here's the current [formula](https://i.imgur.com/9u6JB2h.png)"
)]
#[usage("[match url / match id] [amount of warmups]")]
#[example("58320988 1", "https://osu.ppy.sh/community/matches/58320988")]
#[aliases("mc", "matchcost")]
async fn matchcosts(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = match MatchArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };
    let match_id = args.match_id;
    let warmups = args.warmups;

    // Retrieve the match
    let match_req = MatchRequest::with_match_id(match_id);
    let osu_match = match match_req.queue_single(&ctx.clients.osu).await {
        Ok(osu_match) => osu_match,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    let mode = osu_match
        .games
        .first()
        .map(|game| game.mode)
        .unwrap_or_default();

    // Retrieve all users of the match
    let requests = osu_match
        .games
        .iter()
        .map(|game| game.scores.iter())
        .flatten()
        .map(|s| s.user_id)
        .unique()
        .map(|id| {
            UserRequest::with_user_id(id)
                .mode(mode)
                .queue_single(ctx.osu())
                .map_ok(move |user| (id, user))
        })
        .collect_vec();
    let users: HashMap<_, _> = match try_join_all(requests).await {
        Ok(users) => users
            .into_iter()
            .map(|(id, user)| {
                user.map_or_else(
                    || (id, id.to_string()),
                    |user| (user.user_id, user.username),
                )
            })
            .collect(),
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Process match
    let (description, match_result) = if osu_match.games.len() <= warmups {
        let mut description = String::from("No games played yet");
        if !osu_match.games.is_empty() && warmups > 0 {
            let _ = write!(
                description,
                " beyond the {} warmup{}",
                warmups,
                if warmups > 1 { "s" } else { "" }
            );
        }
        (Some(description), None)
    } else {
        let result = process_match(users.clone(), &osu_match, warmups);
        (None, Some(result))
    };

    // Accumulate all necessary data
    let data = MatchCostEmbed::new(osu_match.clone(), description, match_result);

    // Creating the embed
    let embed = data.build().build()?;
    msg.build_response(&ctx, |mut m| {
        if warmups > 0 {
            let mut content = String::from("Ignoring the first ");
            if warmups == 1 {
                content.push_str("map");
            } else {
                let _ = write!(content, "{} maps", warmups);
            }
            content.push_str(" as warmup:");
            m = m.content(content)?;
        }
        m.embed(embed)
    })
    .await?;
    Ok(())
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

fn process_match(
    mut users: HashMap<u32, String>,
    osu_match: &Match,
    warmups: usize,
) -> MatchResult {
    let games: Vec<_> = osu_match.games.iter().skip(warmups).collect();
    let games_len = games.len() as f32;
    let mut teams = HashMap::new();
    let mut point_costs = HashMap::new();
    let mut mods: HashMap<_, HashSet<_>> = HashMap::new();
    let team_vs = games.first().unwrap().team_type == TeamType::TeamVS;
    let mut match_scores = MatchScores(0, 0);
    // Calculate point scores for each score in each game
    for game in games.iter() {
        let score_sum: u32 = game.scores.iter().map(|s| s.score).sum();
        let avg = score_sum as f32 / game.scores.iter().filter(|s| s.score > 0).count() as f32;
        let mut team_scores = HashMap::new();
        for score in game.scores.iter().filter(|s| s.score > 0) {
            mods.entry(score.user_id)
                .or_default()
                .insert(score.enabled_mods.map(|mods| mods - GameMods::NoFail));
            let point_cost = score.score as f32 / avg + FLAT_PARTICIPATION_BONUS;
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
            .fold((Team::None, 0), |winner, next| {
                if next.1 > winner.1 {
                    next
                } else {
                    winner
                }
            });
        match_scores.incr(winner_team);
    }
    // Tiebreaker bonus
    if osu_match.end_time.is_some() && match_scores.difference() == 1 {
        let game = games.last().unwrap();
        point_costs
            .iter_mut()
            .filter(|(&user_id, _)| game.scores.iter().any(|score| score.user_id == user_id))
            .map(|(_, costs)| costs.last_mut().unwrap())
            .for_each(|value| *value = (*value * TIEBREAKER_BONUS) - FLAT_PARTICIPATION_BONUS);
    }
    // Mod combinations bonus
    let mods: Vec<_> = mods
        .into_iter()
        .filter(|(_, mods)| mods.len() > 2)
        .map(|(id, mods)| (id, mods.len() - 2))
        .collect();
    mods.into_iter().for_each(|(user_id, mods)| {
        let multiplier = 1.0 + mods as f32 * MOD_BONUS;
        point_costs.entry(user_id).and_modify(|point_scores| {
            point_scores
                .iter_mut()
                .for_each(|point_score| *point_score *= multiplier);
        });
    });
    // Calculate match costs by combining point costs
    let mut data = HashMap::new();
    let mut highest_cost = 0.0;
    let mut mvp_id = 0;
    for (user, point_costs) in point_costs {
        let name = users.remove(&user).unwrap();
        let sum: f32 = point_costs.iter().sum();
        let costs_len = point_costs.len() as f32;
        let mut match_cost = sum / costs_len;
        match_cost *= BASE_PARTICIPATION_BONUS
            .powf(((costs_len - 1.0) / (games_len - 1.0)).powf(EXP_PARTICIPATION_BONUS));
        data.entry(*teams.get(&user).unwrap())
            .or_insert_with(Vec::new)
            .push((name, match_cost));
        if match_cost > highest_cost {
            highest_cost = match_cost;
            mvp_id = user;
        }
    }
    let player_comparer =
        |a: &(String, f32), b: &(String, f32)| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal);
    if team_vs {
        let blue = match data.remove(&Team::Blue) {
            Some(mut team) => {
                team.sort_unstable_by(player_comparer);
                team
            }
            None => Vec::new(),
        };
        let red = match data.remove(&Team::Red) {
            Some(mut team) => {
                team.sort_unstable_by(player_comparer);
                team
            }
            None => Vec::new(),
        };
        MatchResult::team(mvp_id, match_scores, blue, red)
    } else {
        let mut players = data.remove(&Team::None).unwrap_or_default();
        players.sort_unstable_by(player_comparer);
        MatchResult::solo(mvp_id, players)
    }
}

type TeamResult = Vec<(String, f32)>;

pub enum MatchResult {
    TeamVS {
        blue: TeamResult,
        red: TeamResult,
        mvp: u32,
        match_scores: MatchScores,
    },
    HeadToHead {
        players: TeamResult,
        mvp: u32,
    },
}

impl MatchResult {
    fn team(mvp: u32, match_scores: MatchScores, blue: TeamResult, red: TeamResult) -> Self {
        Self::TeamVS {
            mvp,
            match_scores,
            blue,
            red,
        }
    }
    fn solo(mvp: u32, players: TeamResult) -> Self {
        Self::HeadToHead { mvp, players }
    }
    pub fn mvp_id(&self) -> u32 {
        match self {
            MatchResult::TeamVS { mvp, .. } => *mvp,
            MatchResult::HeadToHead { mvp, .. } => *mvp,
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
