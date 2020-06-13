use crate::{
    embeds::EmbedData,
    util::{
        globals::{AVATAR_URL, HOMEPAGE},
        numbers::round,
    },
};

use rosu::models::{Match, Team, TeamType};
use std::{collections::HashMap, fmt::Write};

#[derive(Clone)]
pub struct MatchCostEmbed {
    description: String,
    thumbnail: Option<String>,
    title: String,
    url: String,
}

impl MatchCostEmbed {
    pub fn new(mut users: HashMap<u32, String>, osu_match: Match, warmups: usize) -> Self {
        let mut thumbnail = None;
        let mut title_text = osu_match.name;
        title_text.retain(|c| c != '(' && c != ')');
        let description = if osu_match.games.len() <= warmups {
            let mut description = String::from("No games played yet");
            if !osu_match.games.is_empty() && warmups > 0 {
                let _ = write!(
                    description,
                    " beyond the {} warmup{}",
                    warmups,
                    if warmups > 1 { "s" } else { "" }
                );
            }
            description
        } else {
            let games: Vec<_> = osu_match.games.into_iter().skip(warmups).collect();
            let games_len = games.len() as f32;
            let mut teams = HashMap::new();
            let mut point_costs = HashMap::new();
            let team_vs = games.first().unwrap().team_type == TeamType::TeamVS;
            let mut match_scores = MatchScores(0, 0);
            for mut game in games {
                game.scores = game.scores.into_iter().filter(|s| s.score > 0).collect();
                let score_sum: u32 = game.scores.iter().map(|s| s.score).sum();
                let avg = score_sum as f32 / game.scores.len() as f32;
                let mut team_scores = HashMap::new();
                for score in game.scores {
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
            thumbnail = Some(format!("{}{}", AVATAR_URL, mvp_id));
            let player_comparer =
                |a: &(String, f32), b: &(String, f32)| b.1.partial_cmp(&a.1).unwrap();
            let mut description = String::with_capacity(256);
            if team_vs {
                let _ = writeln!(description,
                    "**{word} score:** :blue_circle: {blue_stars}{blue_score}{blue_stars} - {red_stars}{red_score}{red_stars} :red_circle:\n",
                    word = if osu_match.end_time.is_some() { "Final" } else { "Current" },
                    blue_score = match_scores.0,
                    red_score = match_scores.1,
                    blue_stars = if match_scores.0 > match_scores.1 { "**" } else { "" },
                    red_stars = if match_scores.0 < match_scores.1 { "**" } else { "" }, 
                );
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
                let blue_len = blue.len();
                let blue_has_mvp = blue_len > 0
                    && (red.is_empty() || blue.first().unwrap().1 > red.first().unwrap().1);
                if blue_len > 0 {
                    let _ = writeln!(description, ":blue_circle: **Blue Team** :blue_circle:");
                    add_team(&mut description, blue, blue_has_mvp);
                }
                if !red.is_empty() {
                    if blue_len > 0 {
                        description.push('\n');
                    }
                    let _ = writeln!(description, ":red_circle: **Red Team** :red_circle:");
                    add_team(&mut description, red, !blue_has_mvp);
                }
                description
            } else {
                let mut players = data.remove(&Team::None).unwrap_or_default();
                players.sort_by(player_comparer);
                add_team(&mut description, players, true);
                description
            }
        };
        Self {
            description,
            title: title_text,
            url: format!("{}community/matches/{}", HOMEPAGE, osu_match.match_id),
            thumbnail,
        }
    }
}

impl EmbedData for MatchCostEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
}

struct MatchScores(u32, u32);

impl MatchScores {
    fn incr(&mut self, team: Team) {
        match team {
            Team::Blue => self.0 += 1,
            Team::Red => self.1 += 1,
            Team::None => {}
        }
    }
}

fn add_team(description: &mut String, team: Vec<(String, f32)>, with_mvp: bool) {
    for (i, (name, cost)) in team.into_iter().enumerate() {
        let _ = writeln!(
            description,
            "**{idx}**: [{name}]({base}users/{name_r}) - **{cost}**{crown}",
            idx = i + 1,
            name = name,
            base = HOMEPAGE,
            name_r = name.replace(" ", "%20"),
            cost = round(cost),
            crown = if i == 0 && with_mvp { " :crown:" } else { "" },
        );
    }
}
