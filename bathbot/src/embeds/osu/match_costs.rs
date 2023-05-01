use std::{
    borrow::Cow,
    fmt::{Display, Formatter, Result as FmtResult, Write},
    mem,
};

use bathbot_macros::EmbedData;
use bathbot_util::{
    constants::{DESCRIPTION_SIZE, OSU_BASE},
    numbers::WithComma,
    CowUtils, FooterBuilder,
};
use rosu_v2::model::matches::OsuMatch;

use crate::commands::osu::{MatchResult, PlayerResult};

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
        osu_match: &mut OsuMatch,
        description: Option<String>,
        match_result: Option<MatchResult>,
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
                    match_scores,
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
                        blue_score = match_scores.blue(),
                        red_score = match_scores.red(),
                        blue_stars = if match_scores.blue() > match_scores.red() {
                            "**"
                        } else {
                            ""
                        },
                        red_stars = if match_scores.blue() < match_scores.red() {
                            "**"
                        } else {
                            ""
                        },
                    );

                    // Blue team
                    let _ = writeln!(description, ":blue_circle: **Blue Team** :blue_circle:");
                    let mut avg_scores = AverageScores::new(show_scores, &blue);
                    let idx_len = index_len(&blue);

                    for (res, i) in blue.into_iter().zip(1..) {
                        let PlayerResult {
                            user_id,
                            match_cost,
                            avg_score,
                        } = res;

                        let name = match osu_match.users.get(&user_id) {
                            Some(user) => user.username.cow_escape_markdown(),
                            None => Cow::Borrowed("<unknown user>"),
                        };

                        let medal = {
                            let mut idx = 0;

                            while idx < medals.len() {
                                let red_cost =
                                    red.get(idx).map(|res| res.match_cost).unwrap_or(0.0);

                                if match_cost > red_cost {
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

                        let avg_score = avg_scores.get(avg_score);

                        let _ = writeln!(
                            description,
                            "`{i:>idx_len$}.`{avg_score} [{name}]({OSU_BASE}users/{user_id}) - **{match_cost:.2}** {medal}",
                        );
                    }

                    // Red team
                    let _ = writeln!(description, "\n:red_circle: **Red Team** :red_circle:");
                    let mut avg_scores = AverageScores::new(show_scores, &red);
                    let idx_len = index_len(&red);

                    for (res, i) in red.into_iter().zip(1..) {
                        let PlayerResult {
                            user_id,
                            match_cost,
                            avg_score,
                        } = res;

                        let name = match osu_match.users.get(&user_id) {
                            Some(user) => user.username.cow_escape_markdown(),
                            None => Cow::Borrowed("<unknown user>"),
                        };

                        let medal = if !medals.is_empty() {
                            medals.remove(0)
                        } else {
                            ""
                        };

                        let avg_score = avg_scores.get(avg_score);

                        let _ = writeln!(
                            description,
                            "`{i:>idx_len$}.`{avg_score} [{name}]({OSU_BASE}users/{user_id}) - **{match_cost:.2}** {medal}",
                        );
                    }

                    thumbnail = mvp_avatar_url;
                }
                Some(MatchResult::HeadToHead {
                    players,
                    mvp_avatar_url,
                }) => {
                    let mut avg_scores = AverageScores::new(show_scores, &players);
                    let idx_len = index_len(&players);

                    for (res, i) in players.into_iter().zip(1..) {
                        let PlayerResult {
                            user_id,
                            match_cost,
                            avg_score,
                        } = res;

                        let name = match osu_match.users.get(&user_id) {
                            Some(user) => user.username.cow_escape_markdown(),
                            None => Cow::Borrowed("<unknown user>"),
                        };

                        let avg_score = avg_scores.get(avg_score);

                        let _ = writeln!(
                            description,
                            "`{i:>idx_len$}.`{avg_score} [{name}]({OSU_BASE}users/{user_id}) - **{match_cost:.2}** {medal}",
                            medal = medals.get(i - 1).unwrap_or(&""),
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

        let mut title = mem::take(&mut osu_match.name)
            .cow_escape_markdown()
            .into_owned();

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

fn index_len(slice: &[PlayerResult]) -> usize {
    if slice.len() < 10 {
        1
    } else if slice.len() < 100 {
        2
    } else {
        slice.len().to_string().len()
    }
}

struct AverageScores {
    len: usize,
    buf: String,
}

impl AverageScores {
    fn new(show: bool, results: &[PlayerResult]) -> Self {
        if !show {
            return Self {
                len: 0,
                buf: String::new(),
            };
        }

        let mut max = 0;
        let mut buf = String::new();

        for res in results {
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
