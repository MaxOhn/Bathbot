use crate::{
    commands::osu::MatchResult,
    util::{
        builder::FooterBuilder,
        constants::{DESCRIPTION_SIZE, OSU_BASE},
    },
};

use rosu_v2::model::matches::OsuMatch;
use std::{borrow::Cow, fmt::Write};

pub struct MatchCostEmbed {
    description: String,
    thumbnail: String,
    title: String,
    url: String,
    footer: FooterBuilder,
}

impl MatchCostEmbed {
    pub fn new(
        osu_match: &mut OsuMatch,
        description: Option<String>,
        match_result: Option<MatchResult>,
    ) -> Option<Self> {
        let mut thumbnail = String::new();

        let description = if let Some(description) = description {
            description
        } else {
            let mut medals = vec!["🥇", "🥈", "🥉"];
            let mut description = String::with_capacity(256);

            match match_result {
                Some(MatchResult::TeamVS {
                    match_scores,
                    blue,
                    red,
                    mvp_avatar_url,
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
                    let _ = writeln!(description, ":blue_circle: **Blue Team** :blue_circle:");

                    for (i, (id, cost)) in blue.into_iter().enumerate() {
                        let name = match osu_match.users.get(&id) {
                            Some(user) => Cow::Borrowed(user.username.as_str()),
                            None => Cow::Owned(format!("User id {id}")),
                        };

                        let medal = {
                            let mut idx = 0;

                            while idx < medals.len() {
                                let red_cost = red.get(idx).map(|(.., cost)| *cost).unwrap_or(0.0);

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
                            "**{idx}**: [{name}]({base}users/{user_id}) - **{cost:.2}** {medal}",
                            idx = i + 1,
                            name = name,
                            base = OSU_BASE,
                            user_id = id,
                            cost = cost,
                            medal = medal,
                        );
                    }

                    // Red team
                    let _ = writeln!(description, "\n:red_circle: **Red Team** :red_circle:");

                    for (i, (id, cost)) in red.into_iter().enumerate() {
                        let name = match osu_match.users.get(&id) {
                            Some(user) => Cow::Borrowed(user.username.as_str()),
                            None => Cow::Owned(format!("User id {id}")),
                        };

                        let medal = if !medals.is_empty() {
                            medals.remove(0)
                        } else {
                            ""
                        };

                        let _ = writeln!(
                            description,
                            "**{idx}**: [{name}]({base}users/{user_id}) - **{cost:.2}** {medal}",
                            idx = i + 1,
                            name = name,
                            base = OSU_BASE,
                            user_id = id,
                            cost = cost,
                            medal = medal,
                        );
                    }

                    thumbnail = mvp_avatar_url;
                }
                Some(MatchResult::HeadToHead {
                    players,
                    mvp_avatar_url,
                }) => {
                    for (i, (id, cost)) in players.into_iter().enumerate() {
                        let name = match osu_match.users.get(&id) {
                            Some(user) => Cow::Borrowed(user.username.as_str()),
                            None => Cow::Owned(format!("User id {id}")),
                        };

                        let _ = writeln!(
                            description,
                            "**{idx}**: [{name}]({base}users/{user_id}) - **{cost:.2}** {medal}",
                            idx = i + 1,
                            name = name,
                            base = OSU_BASE,
                            user_id = id,
                            cost = cost,
                            medal = medals.get(i).unwrap_or(&""),
                        );
                    }

                    thumbnail = mvp_avatar_url;
                }
                None => unreachable!(),
            }

            if description.len() >= DESCRIPTION_SIZE {
                return None;
            }

            description
        };

        let match_id = osu_match.match_id;
        let mut title = String::new();
        std::mem::swap(&mut title, &mut osu_match.name);
        title.retain(|c| c != '(' && c != ')');
        let footer = FooterBuilder::new("Note: Formula is subject to change; values are volatile");

        Some(Self {
            title,
            footer,
            thumbnail,
            description,
            url: format!("{OSU_BASE}community/matches/{match_id}"),
        })
    }
}

impl_builder!(MatchCostEmbed {
    description,
    footer,
    thumbnail,
    title,
    url,
});
