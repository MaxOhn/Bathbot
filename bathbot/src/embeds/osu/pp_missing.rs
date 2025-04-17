use std::{
    cmp::Ordering,
    convert::identity,
    fmt::{Display, Formatter, Result as FmtResult},
    iter,
};

use bathbot_util::{
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder,
    numbers::WithComma,
    osu::{ExtractablePp, PpListUtil, pp_missing},
};
use rosu_v2::prelude::Score;

use crate::{embeds::EmbedData, manager::redis::osu::CachedUser, util::CachedUserExt};

fn idx_suffix(idx: usize) -> &'static str {
    match idx % 100 {
        11..=13 => "th",
        _ => match idx % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    }
}

pub struct PpMissingEmbed {
    author: AuthorBuilder,
    description: String,
    footer: Option<FooterBuilder>,
    thumbnail: String,
    title: String,
}

impl PpMissingEmbed {
    pub fn new(
        user: &CachedUser,
        scores: &[Score],
        goal_pp: f64,
        rank: Option<u32>,
        each: Option<f32>,
        amount: Option<u8>,
    ) -> Self {
        let stats_pp = user
            .statistics
            .as_ref()
            .expect("missing stats")
            .pp
            .to_native() as f64;

        let username = user.username.as_str();

        let title = format!(
            "What scores is {name} missing to reach {goal_pp}pp?",
            name = username.cow_escape_markdown(),
            goal_pp = WithComma::new(goal_pp),
        );

        let description = match (scores.last().and_then(|s| s.pp), each, amount) {
            // No top scores
            (None, ..) => "No top scores found".to_owned(),
            // Total pp already above goal
            _ if stats_pp > goal_pp => format!(
                "{name} has {pp_raw}pp which is already more than {pp_given}pp.",
                name = username.cow_escape_markdown(),
                pp_raw = WithComma::new(stats_pp),
                pp_given = WithComma::new(goal_pp),
            ),
            // Reach goal with only one score
            (Some(_), None, None | Some(1)) => {
                let (required, idx) = pp_missing(stats_pp, goal_pp, scores);
                let suffix = idx_suffix(idx + 1);

                format!(
                    "To reach {pp}pp with one additional score, {user} needs to perform \
                    a **{required}pp** score which would be their {approx}{idx}{suffix} top play",
                    pp = WithComma::new(goal_pp),
                    user = username.cow_escape_markdown(),
                    required = WithComma::new(required),
                    approx = if idx >= 200 { "~" } else { "" },
                    idx = idx + 1,
                )
            }
            // Given score pp is below last top 200 score pp
            (Some(last_pp), Some(each), _) if each < last_pp => {
                format!(
                    "New top200 scores require at least **{last_pp}pp** for {user} \
                    so {pp} total pp can't be reached with {each}pp scores.",
                    pp = WithComma::new(goal_pp),
                    last_pp = WithComma::new(last_pp),
                    each = WithComma::new(each),
                    user = username.cow_escape_markdown(),
                )
            }
            // Given score pp would be in top 200
            (Some(_), Some(each), _) => {
                let mut pps = scores.extract_pp();
                let (required, idx) = pp_missing(stats_pp, goal_pp, scores);
                let required = required as f32;

                if required < each {
                    let idx = idx + 1;
                    let suffix = idx_suffix(idx);

                    format!(
                        "To reach {pp}pp with one additional score, {user} needs to perform \
                        a **{required}pp** score which would be their {idx}{suffix} top play",
                        pp = WithComma::new(goal_pp),
                        user = username.cow_escape_markdown(),
                        required = WithComma::new(required),
                    )
                } else {
                    let idx = pps.partition_point(|&pp| pp >= each);

                    let mut iter = pps
                        .iter()
                        .copied()
                        .zip(0..)
                        .map(|(pp, i)| pp as f64 * FACTOR.powi(i));

                    let mut top: f64 = (&mut iter).take(idx).sum();
                    let bot: f64 = iter.sum();

                    let bonus_pp = f64::max(stats_pp - (top + bot), 0.0);
                    top += bonus_pp;

                    // requires n_each many new scores of `each` many pp and one additional score
                    fn n_each_needed(
                        top: &mut f64,
                        each: f64,
                        goal_pp: f64,
                        pps: &[f32],
                        idx: usize,
                    ) -> Option<usize> {
                        let len = pps.len();

                        for i in idx..len {
                            let bot = pps[idx..]
                                .iter()
                                .zip(i as i32 + 1..)
                                .fold(0.0, |sum, (pp, i)| sum + *pp as f64 * FACTOR.powi(i));

                            let factor = FACTOR.powi(i as i32);

                            if *top + factor * each + bot >= goal_pp {
                                return Some(i - idx);
                            }

                            *top += factor * each;
                        }

                        let bot = pps[idx..]
                            .iter()
                            .copied()
                            .zip(len as i32..)
                            .fold(0.0, |sum, (pp, i)| sum + pp as f64 * FACTOR.powi(i));

                        *top += bot;

                        (*top >= goal_pp).then_some(len - idx)
                    }

                    if let Some(n_each) = n_each_needed(&mut top, each as f64, goal_pp, &pps, idx) {
                        pps.extend(iter::repeat_n(each, n_each));
                        pps.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

                        let accum = pps.accum_weighted();

                        // Calculate the pp of the missing score after adding `n_each` many `each`
                        // pp scores
                        let total = accum + bonus_pp;
                        let (required, _) = pp_missing(total, goal_pp, pps.as_slice());

                        format!(
                            "To reach {pp}pp, {user} needs to perform **{n_each}** more \
                            {each}pp score{plural} and one **{required}pp** score.",
                            each = WithComma::new(each),
                            plural = if n_each != 1 { "s" } else { "" },
                            pp = WithComma::new(goal_pp),
                            user = username.cow_escape_markdown(),
                            required = WithComma::new(required),
                        )
                    } else {
                        format!(
                            "Filling up {user}'{genitiv} top scores with {amount} new {each}pp score{plural} \
                            would only lead to {approx}**{top}pp** which is still less than {pp}pp.",
                            amount = pps.len() - idx,
                            each = WithComma::new(each),
                            plural = if pps.len() - idx != 1 { "s" } else { "" },
                            genitiv = if idx != 1 { "s" } else { "" },
                            pp = WithComma::new(goal_pp),
                            approx = if idx >= 200 { "roughly " } else { "" },
                            top = WithComma::new(top),
                            user = username.cow_escape_markdown(),
                        )
                    }
                }
            }
            (Some(_), None, Some(amount)) => {
                let pps = scores.extract_pp();

                let raw_delta = goal_pp - stats_pp;
                let weight_sum: f64 = (0..amount as i32).map(|exp| FACTOR.powi(exp)).sum();
                let mid_goal = stats_pp + (raw_delta / weight_sum);
                let (required, _) = pp_missing(stats_pp, mid_goal, pps.as_slice());
                let mut required = required as f32;

                let pb_start_idx = pps
                    .binary_search_by(|probe| required.total_cmp(probe))
                    .map_or_else(identity, |idx| idx + 1);

                let pb_fmt = PersonalBestIndexFormatter::new(pb_start_idx, amount);

                if scores.len() >= 100 && required < *pps.last().unwrap() {
                    required = (*pps.last().unwrap() - 0.01).max(0.0);
                }

                format!(
                    "To reach {pp}pp with {amount} additional score{plural} of \
                    the same pp, each of them would need to be **{required}pp**, \
                    placing them {pb_fmt} for {user}.",
                    pp = WithComma::new(goal_pp),
                    plural = if amount == 1 { "" } else { "s" },
                    required = WithComma::new(required),
                    user = username.cow_escape_markdown(),
                )
            }
        };

        let footer = rank.map(|rank| {
            FooterBuilder::new(format!(
                "The current rank for {pp}pp is approx. #{rank}",
                pp = WithComma::new(goal_pp),
                rank = WithComma::new(rank),
            ))
        });

        Self {
            author: user.author_builder(false),
            description,
            footer,
            thumbnail: user.avatar_url.as_ref().to_owned(),
            title,
        }
    }
}

impl EmbedData for PpMissingEmbed {
    fn build(self) -> EmbedBuilder {
        let builder = EmbedBuilder::new()
            .author(self.author)
            .description(self.description)
            .thumbnail(self.thumbnail)
            .title(self.title);

        if let Some(footer) = self.footer {
            builder.footer(footer)
        } else {
            builder
        }
    }
}

pub struct PersonalBestIndexFormatter {
    start_idx: usize,
    amount: u8,
}

impl PersonalBestIndexFormatter {
    pub fn new(start_idx: usize, amount: u8) -> Self {
        Self { start_idx, amount }
    }
}

impl Display for PersonalBestIndexFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.start_idx >= 200 {
            f.write_str("outside of the top 200")
        } else {
            write!(
                f,
                "at personal bests #{} to #{}",
                self.start_idx + 1,
                self.start_idx + usize::from(self.amount)
            )
        }
    }
}

const FACTOR: f64 = 0.95;
