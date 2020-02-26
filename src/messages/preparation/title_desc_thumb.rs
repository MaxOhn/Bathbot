use crate::util::{
    globals::{AVATAR_URL, HOMEPAGE},
    numbers::round,
};

use rosu::models::{Match, Team, TeamType};
use std::{collections::HashMap, fmt::Write};

pub struct TitleDescThumbData {
    pub title_url: String,
    pub title_text: String,
    pub thumbnail: Option<String>,
    pub description: String,
}

impl TitleDescThumbData {
    pub fn create_match_costs(
        mut users: HashMap<u32, String>,
        osu_match: Match,
        warmups: usize,
    ) -> Self {
        let mut thumbnail = None;
        let title_url = format!("{}community/matches/{}", HOMEPAGE, osu_match.match_id);
        let mut title_text = osu_match.name;
        title_text.retain(|c| c != '(' && c != ')');
        let description = if osu_match.games.len() <= warmups {
            let mut description = String::from("No games played yet");
            if osu_match.games.len() > 0 && warmups > 0 {
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
            for game in games {
                let score_sum: u32 = game.scores.iter().map(|s| s.score).sum();
                let avg = score_sum as f32 / game.scores.len() as f32;
                for score in game.scores {
                    let point_cost = score.score as f32 / avg + 0.4;
                    point_costs
                        .entry(score.user_id)
                        .or_insert_with(Vec::new)
                        .push(point_cost);
                    teams.entry(score.user_id).or_insert(score.team);
                }
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
                let blue_has_mvp = blue.len() > 0
                    && (red.len() == 0 || blue.first().unwrap().1 > red.first().unwrap().1);
                let blue_len = blue.len();
                if blue_len > 0 {
                    let _ = writeln!(description, ":blue_circle: **Blue Team** :blue_circle:");
                    add_team(&mut description, blue, blue_has_mvp);
                }
                if red.len() > 0 {
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
            title_url,
            title_text,
            thumbnail,
            description,
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
            name_r = name.replace(" ", "+"),
            cost = round(cost),
            crown = if i == 0 && with_mvp { " :crown:" } else { "" },
        );
    }
}
