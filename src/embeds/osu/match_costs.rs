use crate::{
    commands::osu::MatchResult,
    embeds::{EmbedData, Footer},
    util::constants::{AVATAR_URL, DESCRIPTION_SIZE, OSU_BASE},
};

use rosu::model::Match;
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct MatchCostEmbed {
    description: Option<String>,
    thumbnail: Option<ImageSource>,
    title: Option<String>,
    url: Option<String>,
    footer: Option<Footer>,
}

impl MatchCostEmbed {
    pub fn new(
        osu_match: Match,
        description: Option<String>,
        match_result: Option<MatchResult>,
    ) -> Result<Self, ()> {
        let mut thumbnail = None;
        let description = if let Some(description) = description {
            description
        } else {
            thumbnail = Some(
                ImageSource::url(format!(
                    "{}{}",
                    AVATAR_URL,
                    match_result.as_ref().unwrap().mvp_id()
                ))
                .unwrap(),
            );
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
                            "**{idx}**: [{name}]({base}users/{name_r}) - **{cost:.2}** {medal}",
                            idx = i + 1,
                            name = name,
                            base = OSU_BASE,
                            name_r = name.replace(" ", "%20"),
                            cost = cost,
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
                            "**{idx}**: [{name}]({base}users/{name_r}) - **{cost:.2}** {medal}",
                            idx = i + 1,
                            name = name,
                            base = OSU_BASE,
                            name_r = name.replace(" ", "%20"),
                            cost = cost,
                            medal = medal,
                        );
                    }
                }
                Some(MatchResult::HeadToHead { players, .. }) => {
                    for (i, (name, cost)) in players.into_iter().enumerate() {
                        let _ = writeln!(
                            description,
                            "**{idx}**: [{name}]({base}users/{name_r}) - **{cost:.2}** {medal}",
                            idx = i + 1,
                            name = name,
                            base = OSU_BASE,
                            name_r = name.replace(" ", "%20"),
                            cost = cost,
                            medal = if i < medals.len() { medals[i] } else { "" },
                        );
                    }
                }
                None => unreachable!(),
            }
            if description.len() >= DESCRIPTION_SIZE {
                return Err(());
            }
            description
        };
        let match_id = osu_match.match_id;
        let mut title = osu_match.name;
        title.retain(|c| c != '(' && c != ')');
        let footer = Footer::new("Note: Formula is subject to change; values are volatile");
        Ok(Self {
            title: Some(title),
            footer: Some(footer),
            thumbnail,
            description: Some(description),
            url: Some(format!("{}community/matches/{}", OSU_BASE, match_id)),
        })
    }
}

impl EmbedData for MatchCostEmbed {
    fn description_owned(&mut self) -> Option<String> {
        self.description.take()
    }
    fn thumbnail_owned(&mut self) -> Option<ImageSource> {
        self.thumbnail.take()
    }
    fn title_owned(&mut self) -> Option<String> {
        self.title.take()
    }
    fn url_owned(&mut self) -> Option<String> {
        self.url.take()
    }
    fn footer_owned(&mut self) -> Option<Footer> {
        self.footer.take()
    }
}
