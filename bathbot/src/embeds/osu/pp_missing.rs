use std::{cmp::Ordering, iter};

use bathbot_model::rosu_v2::user::User;
use bathbot_util::{
    numbers::WithComma,
    osu::{approx_more_pp, pp_missing, ExtractablePp, PpListUtil},
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder,
};
use rosu_v2::prelude::Score;

use crate::{embeds::EmbedData, manager::redis::RedisData};

pub struct PpMissingEmbed {
    author: AuthorBuilder,
    description: String,
    footer: Option<FooterBuilder>,
    thumbnail: String,
    title: String,
}

impl PpMissingEmbed {
    pub fn new(
        user: &RedisData<User>,
        scores: &[Score],
        goal_pp: f32,
        rank: Option<u32>,
        each: Option<f32>,
    ) -> Self {
        let stats_pp = user.stats().pp();

        let username = user.username();

        let title = format!(
            "What scores is {name} missing to reach {goal_pp}pp?",
            name = username.cow_escape_markdown(),
            goal_pp = WithComma::new(goal_pp),
        );

        let description = match (scores.last().and_then(|s| s.pp), each) {
            // No top scores
            (None, _) => "No top scores found".to_owned(),
            // Total pp already above goal
            _ if stats_pp > goal_pp => format!(
                "{name} has {pp_raw}pp which is already more than {pp_given}pp.",
                name = username.cow_escape_markdown(),
                pp_raw = WithComma::new(stats_pp),
                pp_given = WithComma::new(goal_pp),
            ),
            // Reach goal with only one score
            (Some(_), None) => {
                let (required, idx) = if scores.len() == 100 {
                    let mut pps = scores.extract_pp();
                    approx_more_pp(&mut pps, 50);

                    let (mut required, mut idx) = pp_missing(stats_pp, goal_pp, pps.as_slice());

                    // Instead of using the approximation too literally, max
                    // out on the 100th top score.
                    let top100 = pps[99];

                    if top100 > required {
                        required = top100;
                        idx = 99;
                    }

                    (required, idx)
                } else {
                    pp_missing(stats_pp, goal_pp, scores)
                };

                format!(
                    "To reach {pp}pp with one additional score, {user} needs to perform \
                    a **{required}pp** score which would be the top {approx}#{idx}",
                    pp = WithComma::new(goal_pp),
                    user = username.cow_escape_markdown(),
                    required = WithComma::new(required),
                    approx = if idx >= 100 { "~" } else { "" },
                    idx = idx + 1,
                )
            }
            // Given score pp is below last top 100 score pp
            (Some(last_pp), Some(each)) if each < last_pp => {
                format!(
                    "New top100 scores require at least **{last_pp}pp** for {user} \
                    so {pp} total pp can't be reached with {each}pp scores.",
                    pp = WithComma::new(goal_pp),
                    last_pp = WithComma::new(last_pp),
                    each = WithComma::new(each),
                    user = username.cow_escape_markdown(),
                )
            }
            // Given score pp would be in top 100
            (Some(_), Some(each)) => {
                let mut pps = scores.extract_pp();

                let (required, idx) = if scores.len() == 100 {
                    approx_more_pp(&mut pps, 50);

                    let (mut required, mut idx) = pp_missing(stats_pp, goal_pp, pps.as_slice());

                    // Instead of using the approximation too literally, max
                    // out on the 100th top score.
                    let top100 = pps[99];

                    if top100 > required {
                        required = top100;
                        idx = 99;
                    }

                    (required, idx)
                } else {
                    pp_missing(stats_pp, goal_pp, scores)
                };

                if required < each {
                    format!(
                        "To reach {pp}pp with one additional score, {user} needs to perform \
                        a **{required}pp** score which would be the top #{idx}",
                        pp = WithComma::new(goal_pp),
                        user = username.cow_escape_markdown(),
                        required = WithComma::new(required),
                        idx = idx + 1,
                    )
                } else {
                    let idx = pps.partition_point(|&pp| pp >= each);

                    let mut iter = pps
                        .iter()
                        .copied()
                        .zip(0..)
                        .map(|(pp, i)| pp * 0.95_f32.powi(i));

                    let mut top: f32 = (&mut iter).take(idx).sum();
                    let bot: f32 = iter.sum();

                    let bonus_pp = (stats_pp - (top + bot)).max(0.0);
                    top += bonus_pp;

                    // requires n_each many new scores of `each` many pp and one additional score
                    fn n_each_needed(
                        top: &mut f32,
                        each: f32,
                        goal_pp: f32,
                        pps: &[f32],
                        idx: usize,
                    ) -> Option<usize> {
                        let len = pps.len();

                        for i in idx..len {
                            let bot = pps[idx..]
                                .iter()
                                .copied()
                                .zip(i as i32 + 1..)
                                .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i));

                            let factor = 0.95_f32.powi(i as i32);

                            if *top + factor * each + bot >= goal_pp {
                                return Some(i - idx);
                            }

                            *top += factor * each;
                        }

                        let bot = pps[idx..]
                            .iter()
                            .copied()
                            .zip(len as i32..)
                            .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i));

                        *top += bot;

                        (*top >= goal_pp).then_some(len - idx)
                    }

                    if let Some(n_each) = n_each_needed(&mut top, each, goal_pp, &pps, idx) {
                        pps.extend(iter::repeat(each).take(n_each));
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
                            approx = if idx >= 100 { "roughly " } else { "" },
                            top = WithComma::new(top),
                            user = username.cow_escape_markdown(),
                        )
                    }
                }
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
            author: user.author_builder(),
            description,
            footer,
            thumbnail: user.avatar_url().to_owned(),
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
