use std::{
    borrow::Cow,
    fmt::{Display, Formatter, Result as FmtResult, Write},
};

use bathbot_macros::EmbedData;
use bathbot_util::{
    constants::{DESCRIPTION_SIZE, OSU_BASE},
    numbers::WithComma,
    CowUtils, FooterBuilder,
};
use rosu_v2::model::matches::OsuMatch;

use crate::commands::osu::{MatchResult, UserMatchCostEntry};

#[derive(EmbedData)]
pub struct MatchCostEmbed {
    description: String,
    thumbnail: String,
    title: String,
    url: String,
    footer: FooterBuilder,
}

impl MatchCostEmbed {
    pub fn new(
        osu_match: &OsuMatch,
        description: Option<String>,
        match_result: Option<MatchResult<'_>>,
        show_scores: bool,
    ) -> Option<Self> {
        let mut thumbnail = String::new();

        let description = if let Some(description) = description {
            description
        } else {
            let mut medals = vec!["🥇", "🥈", "🥉"];
            let mut description = String::with_capacity(256);

            match match_result {
                Some(MatchResult::TeamVS {
                    blue,
                    red,
                    mvp_avatar_url,
                }) => {
                    // "Title"
                    let _ = writeln!(
                        description,
                        "**{word} score:** :blue_circle: {blue_stars}{blue_score}{blue_stars} \
                        - {red_stars}{red_score}{red_stars} :red_circle:\n",
                        word = if osu_match.end_time.is_some() {
                            "Final"
                        } else {
                            "Current"
                        },
                        blue_score = blue.win_count,
                        red_score = red.win_count,
                        blue_stars = if blue.win_count > red.win_count {
                            "**"
                        } else {
                            ""
                        },
                        red_stars = if blue.win_count < red.win_count {
                            "**"
                        } else {
                            ""
                        },
                    );

                    // Blue team
                    let _ = writeln!(description, ":blue_circle: **Blue Team** :blue_circle:");
                    let mut avg_scores = AverageScores::new(show_scores, &blue.players);
                    let idx_len = index_len(&blue.players);

                    for (entry, i) in blue.players.iter().zip(1..) {
                        let UserMatchCostEntry {
                            user_id,
                            point_cost,
                            participation_bonus_factor,
                            mods_bonus_factor,
                            tiebreaker_bonus,
                            match_cost,
                            avg_score,
                        } = entry;

                        let name = match osu_match.users.get(user_id) {
                            Some(user) => user.username.cow_escape_markdown(),
                            None => Cow::Borrowed("<unknown user>"),
                        };

                        let medal = {
                            let mut idx = 0;

                            while idx < medals.len() {
                                let red_cost = red
                                    .players
                                    .get(idx)
                                    .map(|res| res.match_cost)
                                    .unwrap_or(0.0);

                                if *match_cost > red_cost {
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

                        let avg_score = avg_scores.get(*avg_score);

                        let _ = writeln!(
                            description,
                            "`{i:>idx_len$}.`{avg_score} [{name}]({OSU_BASE}users/{user_id}) - **{match_cost:.2}** {medal}",
                        );
                    }

                    // Red team
                    let _ = writeln!(description, "\n:red_circle: **Red Team** :red_circle:");
                    let mut avg_scores = AverageScores::new(show_scores, &red.players);
                    let idx_len = index_len(&red.players);

                    for (entry, i) in red.players.iter().zip(1..) {
                        let UserMatchCostEntry {
                            user_id,
                            point_cost,
                            participation_bonus_factor,
                            mods_bonus_factor,
                            tiebreaker_bonus,
                            match_cost,
                            avg_score,
                        } = entry;

                        let name = match osu_match.users.get(user_id) {
                            Some(user) => user.username.cow_escape_markdown(),
                            None => Cow::Borrowed("<unknown user>"),
                        };

                        let medal = if !medals.is_empty() {
                            medals.remove(0)
                        } else {
                            ""
                        };

                        let avg_score = avg_scores.get(*avg_score);

                        let _ = writeln!(
                            description,
                            "`{i:>idx_len$}.`{avg_score} [{name}]({OSU_BASE}users/{user_id}) - **{match_cost:.2}** {medal}",
                        );
                    }

                    thumbnail = mvp_avatar_url.to_owned();
                }
                Some(MatchResult::HeadToHead {
                    players,
                    mvp_avatar_url,
                }) => {
                    let mut avg_scores = AverageScores::new(show_scores, &players);
                    let idx_len = index_len(&players);

                    for (entry, i) in players.iter().zip(1..) {
                        let UserMatchCostEntry {
                            user_id,
                            point_cost,
                            participation_bonus_factor,
                            mods_bonus_factor,
                            tiebreaker_bonus,
                            match_cost,
                            avg_score,
                        } = entry;

                        let name = match osu_match.users.get(user_id) {
                            Some(user) => user.username.cow_escape_markdown(),
                            None => Cow::Borrowed("<unknown user>"),
                        };

                        let avg_score = avg_scores.get(*avg_score);

                        let _ = writeln!(
                            description,
                            "`{i:>idx_len$}.`{avg_score} [{name}]({OSU_BASE}users/{user_id}) - **{match_cost:.2}** {medal}",
                            medal = medals.get(i - 1).unwrap_or(&""),
                        );
                    }

                    thumbnail = mvp_avatar_url.to_owned();
                }
                None => unreachable!(),
            }

            if description.len() >= DESCRIPTION_SIZE {
                return None;
            }

            description
        };

        let match_id = osu_match.match_id;
        let mut title = osu_match.name.clone().cow_escape_markdown().into_owned();

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

fn index_len(entries: &[UserMatchCostEntry]) -> usize {
    if entries.len() < 10 {
        1
    } else if entries.len() < 100 {
        2
    } else {
        entries.len().to_string().len()
    }
}

struct AverageScores {
    len: usize,
    buf: String,
}

impl AverageScores {
    fn new(show: bool, entries: &[UserMatchCostEntry]) -> Self {
        if !show {
            return Self {
                len: 0,
                buf: String::new(),
            };
        }

        let mut max = 0;
        let mut buf = String::new();

        for res in entries {
            buf.clear();
            let _ = write!(buf, "{}", WithComma::new(res.avg_score));
            max = max.max(buf.len());
        }

        Self { buf, len: max }
    }

    fn get(&mut self, score: u32) -> AverageScoreFormatter<'_> {
        let score = if self.len == 0 {
            ""
        } else {
            self.buf.clear();
            let _ = write!(self.buf, "{}", WithComma::new(score));

            &self.buf
        };

        AverageScoreFormatter::new(score, self.len)
    }
}

struct AverageScoreFormatter<'s> {
    score: &'s str,
    len: usize,
}

impl<'s> AverageScoreFormatter<'s> {
    fn new(score: &'s str, len: usize) -> Self {
        Self { score, len }
    }
}

impl Display for AverageScoreFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.len == 0 {
            Ok(())
        } else {
            write!(f, " `{score:>len$}`", score = self.score, len = self.len)
        }
    }
}
