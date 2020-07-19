use crate::{
    commands::osu::MatchResult,
    embeds::EmbedData,
    util::{
        globals::{AVATAR_URL, OSU_BASE},
        numbers::round,
    },
};

use rosu::models::Match;
use std::fmt::Write;

#[derive(Clone)]
pub struct MatchCostEmbed {
    description: String,
    thumbnail: Option<String>,
    title: String,
    url: String,
}

impl MatchCostEmbed {
    pub fn new(
        osu_match: Match,
        description: Option<String>,
        match_result: Option<MatchResult>,
    ) -> Self {
        let mut thumbnail = None;
        let description = if let Some(description) = description {
            description
        } else {
            thumbnail = Some(format!(
                "{}{}",
                AVATAR_URL,
                match_result.as_ref().unwrap().mvp_id()
            ));
            let medals = ["🥇", "🥈", "🥉"];
            let mut description = String::with_capacity(256);
            match match_result {
                Some(MatchResult::TeamVS {
                    match_scores,
                    blue,
                    red,
                    ..
                }) => {
                    // "Title"
                    let _ = writeln!(description,
                        "**{word} score:** :blue_circle: {blue_stars}{blue_score}{blue_stars} - {red_stars}{red_score}{red_stars} :red_circle:\n",
                        word = if osu_match.end_time.is_some() { "Final" } else { "Current" },
                        blue_score = match_scores.blue(),
                        red_score = match_scores.red(),
                        blue_stars = if match_scores.blue() > match_scores.red() { "**" } else { "" },
                        red_stars = if match_scores.blue() < match_scores.red() { "**" } else { "" },
                    );
                    // Blue team
                    let mut medals = medals.to_vec();
                    let _ = writeln!(description, ":blue_circle: **Blue Team** :blue_circle:");
                    for (i, (name, cost)) in blue.into_iter().enumerate() {
                        let medal = {
                            let mut idx = 0;
                            while idx < medals.len() {
                                let red_cost = red.get(idx).map(|(_, cost)| *cost).unwrap_or(0.0);
                                if cost > red_cost {
                                    break;
                                }
                                idx += 1;
                            }
                            if idx < medals.len() {
                                medals.remove(idx)
                            } else {
                                ""
                            }
                        };
                        let _ = writeln!(
                            description,
                            "**{idx}**: [{name}]({base}users/{name_r}) - **{cost}** {medal}",
                            idx = i + 1,
                            name = name,
                            base = HOMEPAGE,
                            name_r = name.replace(" ", "%20"),
                            cost = round(cost),
                            medal = medal,
                        );
                    }
                    // Red team
                    let _ = writeln!(description, "\n:red_circle: **Red Team** :red_circle:");
                    for (i, (name, cost)) in red.into_iter().enumerate() {
                        let medal = if !medals.is_empty() {
                            medals.remove(0)
                        } else {
                            ""
                        };
                        let _ = writeln!(
                            description,
                            "**{idx}**: [{name}]({base}users/{name_r}) - **{cost}** {medal}",
                            idx = i + 1,
                            name = name,
                            base = HOMEPAGE,
                            name_r = name.replace(" ", "%20"),
                            cost = round(cost),
                            medal = medal,
                        );
                    }
                }
                Some(MatchResult::HeadToHead { players, .. }) => {
                    for (i, (name, cost)) in players.into_iter().enumerate() {
                        let _ = writeln!(
                            description,
                            "**{idx}**: [{name}]({base}users/{name_r}) - **{cost}** {medal}",
                            idx = i + 1,
                            name = name,
                            base = HOMEPAGE,
                            name_r = name.replace(" ", "%20"),
                            cost = round(cost),
                            medal = if i < medals.len() { medals[i] } else { "" },
                        );
                    }
                }
                None => unreachable!(),
            }
            description
        };
        let match_id = osu_match.match_id;
        let mut title = osu_match.name;
        title.retain(|c| c != '(' && c != ')');
        Self {
            title,
            thumbnail,
            description,
            url: format!("{}community/matches/{}", HOMEPAGE, match_id),
        }
    }
}

impl EmbedData for MatchCostEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn thumbnail(&self) -> Option<&str> {
        self.thumbnail.as_deref()
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn url(&self) -> Option<&str> {
        Some(&self.url)
    }
}
