use crate::{
    arguments::{Args, MatchArgs},
    embeds::{EmbedData, MatchCostEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu::{
    backend::requests::{MatchRequest, UserRequest},
    models::{Match, Team, TeamType},
};
use std::{collections::HashMap, fmt::Write};
use twilight::model::channel::Message;

#[command]
#[short_desc("Display performance rating for a multiplayer match")]
#[long_desc(
    "Calculate a performance rating for each player \
     in the given multiplayer match. The optional second \
     argument is the amount of played warmups, defaults to 2.\n\
     More info over at https://github.com/dain98/Minccino#faq"
)]
#[usage("[match url / match id] [amount of warmups]")]
#[example("58320988 1")]
#[example("https://osu.ppy.sh/community/matches/58320988")]
#[aliases("mc", "matchcost")]
async fn matchcosts(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = match MatchArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => {
            msg.respond(&ctx, err_msg).await?;
            return Ok(());
        }
    };
    let match_id = args.match_id;
    let warmups = args.warmups;

    // Retrieve the match
    let mut osu_match = {
        let match_req = MatchRequest::with_match_id(match_id);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        match match_req.queue_single(&osu).await {
            Ok(osu_match) => osu_match,
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        }
    };

    // Retrieve all usernames of the match
    let users: HashMap<u32, String> = {
        let mut users = HashMap::new();
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        for game in osu_match.games.iter() {
            #[allow(clippy::map_entry)]
            for score in game.scores.iter() {
                if !users.contains_key(&score.user_id) {
                    let req = UserRequest::with_user_id(score.user_id);
                    let name = match req.queue_single(&osu).await {
                        Ok(result) => match result {
                            Some(user) => user.username,
                            None => score.user_id.to_string(),
                        },
                        Err(why) => {
                            msg.respond(&ctx, OSU_API_ISSUE).await?;
                            return Err(why.into());
                        }
                    };
                    users.insert(score.user_id, name);
                }
            }
        }
        users
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
        let result = process_match(users, &mut osu_match, warmups);
        (None, Some(result))
    };

    // Accumulate all necessary data
    let data = MatchCostEmbed::new(osu_match, description, match_result);

    // Creating the embed
    msg.channel_id
        .send_message(ctx, |m| {
            if warmups > 0 {
                let mut content = String::from("Ignoring the first ");
                if warmups == 1 {
                    content.push_str("map");
                } else {
                    let _ = write!(content, "{} maps", warmups);
                }
                content.push_str(" as warmup:");
                m.content(content);
            }
            m.embed(|e| data.build(e))
        })
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}

fn process_match(
    mut users: HashMap<u32, String>,
    r#match: &mut Match,
    warmups: usize,
) -> MatchResult {
    let games: Vec<_> = r#match.games.iter_mut().skip(warmups).collect();
    let games_len = games.len() as f32;
    let mut teams = HashMap::new();
    let mut point_costs = HashMap::new();
    let team_vs = games.first().unwrap().team_type == TeamType::TeamVS;
    let mut match_scores = MatchScores(0, 0);
    for game in games {
        let score_sum: u32 = game.scores.iter().map(|s| s.score).sum();
        let avg = score_sum as f32 / game.scores.iter().filter(|s| s.score > 0).count() as f32;
        let mut team_scores = HashMap::new();
        for score in game.scores.iter().filter(|s| s.score > 0) {
            let point_cost = score.score as f32 / avg + 0.4;
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
        let winner_team = team_scores
            .into_iter()
            .fold((Team::None, 0), |winner, next| {
                if next.1 > winner.1 {
                    next
                } else {
                    winner
                }
            })
            .0;
        match_scores.incr(winner_team);
    }
    let mut data = HashMap::new();
    let mut highest_cost = 0.0;
    let mut mvp_id = 0;
    for (user, point_costs) in point_costs {
        let name = users.remove(&user).unwrap();
        let sum: f32 = point_costs.iter().sum();
        let costs_len = point_costs.len() as f32;
        let mut match_cost = sum / costs_len;
        match_cost *= 1.2_f32.powf((costs_len / games_len).powf(0.4));
        data.entry(*teams.get(&user).unwrap())
            .or_insert_with(Vec::new)
            .push((name, match_cost));
        if match_cost > highest_cost {
            highest_cost = match_cost;
            mvp_id = user;
        }
    }
    let player_comparer = |a: &(String, f32), b: &(String, f32)| b.1.partial_cmp(&a.1).unwrap();
    if team_vs {
        let blue = match data.remove(&Team::Blue) {
            Some(mut team) => {
                team.sort_by(player_comparer);
                team
            }
            None => Vec::new(),
        };
        let red = match data.remove(&Team::Red) {
            Some(mut team) => {
                team.sort_by(player_comparer);
                team
            }
            None => Vec::new(),
        };
        MatchResult::team(mvp_id, match_scores, blue, red)
    } else {
        let mut players = data.remove(&Team::None).unwrap_or_default();
        players.sort_by(player_comparer);
        MatchResult::solo(mvp_id, players)
    }
}

type TeamResult = Vec<(String, f32)>;

pub enum MatchResult {
    #[allow(dead_code)] // c'mon rust...
    TeamVS {
        blue: TeamResult,
        red: TeamResult,
        mvp: u32,
        match_scores: MatchScores,
    },
    #[allow(dead_code)]
    HeadToHead { players: TeamResult, mvp: u32 },
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
}
