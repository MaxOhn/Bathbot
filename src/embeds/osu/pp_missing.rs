use std::{cmp::Ordering, iter};

use rosu_v2::prelude::{Score, User};

use crate::{
    embeds::{Author, EmbedBuilder, EmbedData, Footer},
    util::{
        numbers::{with_comma_float, with_comma_int},
        osu::pp_missing,
    },
};

pub struct PPMissingEmbed {
    author: Author,
    description: String,
    footer: Option<Footer>,
    thumbnail: String,
    title: String,
}

impl PPMissingEmbed {
    pub fn new(
        user: User,
        scores: &mut [Score],
        goal_pp: f32,
        rank: Option<usize>,
        each: Option<f32>,
    ) -> Self {
        let stats_pp = user.statistics.as_ref().unwrap().pp;

        let title = format!(
            "What scores is {name} missing to reach {goal_pp}pp?",
            name = user.username,
            goal_pp = with_comma_float(goal_pp),
        );

        let description = match (scores.last().and_then(|s| s.pp), each) {
            // No top scores
            (None, _) => format!("No top scores found"),
            // Total pp already above goal
            _ if stats_pp > goal_pp => format!(
                "{name} has {pp_raw}pp which is already more than {pp_given}pp.",
                name = user.username,
                pp_raw = with_comma_float(stats_pp),
                pp_given = with_comma_float(goal_pp),
            ),
            // Reach goal with only one score
            (Some(_), None) => {
                let (required, idx) = pp_missing(stats_pp, goal_pp, &(*scores)[..]);

                format!(
                    "To reach {pp}pp with one additional score, {user} needs to perform \
                    a **{required}pp** score which would be the top #{idx}",
                    pp = with_comma_float(goal_pp),
                    user = user.username,
                    required = with_comma_float(required),
                )
            }
            // Top 100 is not full
            (_, Some(each)) if scores.len() < 100 => {
                let idx = scores
                    .iter()
                    .position(|s| s.pp.unwrap_or(0.0) < each)
                    .unwrap_or_else(|| scores.len());

                let mut iter = scores
                    .iter()
                    .filter_map(|s| s.weight.as_ref())
                    .map(|w| w.pp);

                let mut top: f32 = (&mut iter).take(idx).sum();
                let bot: f32 = iter.sum();

                let bonus_pp = stats_pp - (top + bot);
                top += bonus_pp;
                let len = scores.len();

                let mut n_each = 100;

                for i in idx.. {
                    let bot: f32 = scores
                        .iter_mut()
                        .skip(idx)
                        .filter_map(|s| s.weight.as_mut())
                        .map(|w| {
                            w.pp *= 0.95;

                            w.pp
                        })
                        .sum();

                    let factor = 0.95_f32.powi(i as i32);

                    if top + factor * each + bot >= goal_pp {
                        // requires n_each many new scores of `each` many pp and one additional score
                        n_each = i - idx;
                        break;
                    }

                    top += factor * each;
                }

                if n_each == 100 {
                    format!(
                        "Filling up {user}'{genitiv} top100 with {amount} new {each}pp score{plural} \
                        would only lead to **{top}pp** which is still less than {pp}pp.",
                        amount = 100 - len,
                        each = with_comma_float(each),
                        plural = if 100 - len != 1 { "s" } else { "" },
                        genitiv = if idx != 1 { "s" } else { "" },
                        pp = with_comma_float(goal_pp),
                        top = with_comma_float(top),
                        user = user.username,
                    )
                } else {
                    // Add `n_each` many `each` pp scores
                    let mut pps: Vec<_> = scores
                        .iter()
                        .filter_map(|s| s.pp)
                        .chain(iter::repeat(each))
                        .take((len + n_each).min(100))
                        .collect();

                    pps.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

                    let total = pps
                        .iter()
                        .enumerate()
                        .fold(0.0, |sum, (i, next)| sum + next * 0.95_f32.powi(i as i32))
                        + bonus_pp;

                    // Calculate the pp of the missing score
                    let (required, _) = pp_missing(total, goal_pp, pps.as_slice());

                    format!(
                        "To reach {pp}pp, {user} needs to perform **{n_each}** more \
                        {each}pp score{plural} and one **{required}pp** score.",
                        each = with_comma_float(each),
                        plural = if n_each != 1 { "s" } else { "" },
                        pp = with_comma_float(goal_pp),
                        user = user.username,
                        required = with_comma_float(required),
                    )
                }
            }
            // Given score pp is below last top 100 score pp
            (Some(last_pp), Some(each)) if each < last_pp => {
                format!(
                    "New top100 scores require at least **{last_pp}pp** for {user} \
                    so {pp} total pp can't be reached with {each}pp scores.",
                    pp = with_comma_float(goal_pp),
                    last_pp = with_comma_float(last_pp),
                    each = with_comma_float(each),
                    user = user.username,
                )
            }
            // Top 100 is full and given score pp would be in top 100
            (Some(_), Some(each)) => {
                let (required, idx) = pp_missing(stats_pp, goal_pp, &(*scores)[..]);

                if required < each {
                    format!(
                        "To reach {pp}pp with one additional score, {user} needs to perform \
                        a **{required}pp** score which would be the top #{idx}",
                        pp = with_comma_float(goal_pp),
                        user = user.username,
                        required = with_comma_float(required),
                    )
                } else {
                    let idx = scores
                        .iter()
                        .position(|s| s.pp.unwrap_or(0.0) < each)
                        .unwrap_or_else(|| scores.len());

                    let mut iter = scores
                        .iter()
                        .filter_map(|s| s.weight.as_ref())
                        .map(|w| w.pp);

                    let mut top: f32 = (&mut iter).take(idx).sum();
                    let bot: f32 = iter.sum();

                    let bonus_pp = stats_pp - (top + bot);
                    top += bonus_pp;
                    let len = scores.len();

                    let mut n_each = 100;

                    for i in idx..len - idx {
                        let bot: f32 = scores
                            .iter_mut()
                            .skip(idx)
                            .take(len - i - 1)
                            .filter_map(|s| s.weight.as_mut())
                            .map(|w| {
                                w.pp *= 0.95;

                                w.pp
                            })
                            .sum();

                        let factor = 0.95_f32.powi(i as i32);

                        if top + factor * each + bot >= goal_pp {
                            // requires n_each many new scores of `each` many pp and one additional score
                            n_each = i - idx;
                            break;
                        }

                        top += factor * each;
                    }

                    if n_each == 100 {
                        format!(
                            "Filling up {user}'{genitiv} top100 with {amount} new {each}pp score{plural} \
                            would only lead to **{top}pp** which is still less than {pp}pp.",
                            amount = len - idx,
                            each = with_comma_float(each),
                            plural = if len - idx != 1 { "s" } else { "" },
                            genitiv = if idx != 1 { "s" } else { "" },
                            pp = with_comma_float(goal_pp),
                            top = with_comma_float(top),
                            user = user.username,
                        )
                    } else {
                        // Shift scores to right and then overwrite pp values with top_pp
                        scores[idx..].rotate_right(n_each);

                        scores
                            .iter_mut()
                            .skip(idx)
                            .take(n_each)
                            .for_each(|s| s.pp = Some(each));

                        let bot: f32 = scores
                            .iter()
                            .skip(idx + n_each)
                            .filter_map(|s| s.weight.as_ref())
                            .map(|w| w.pp / 0.95)
                            .sum();

                        // Calculate the pp of the missing score after adding `n_each` many `each` pp scores
                        let total = top + bot;
                        let (required, _) = pp_missing(total, goal_pp, &(*scores)[..]);

                        format!(
                            "To reach {pp}pp, {user} needs to perform **{n_each}** more \
                            {each}pp score{plural} and one **{required}pp** score.",
                            each = with_comma_float(each),
                            plural = if n_each != 1 { "s" } else { "" },
                            pp = with_comma_float(goal_pp),
                            user = user.username,
                            required = with_comma_float(required),
                        )
                    }
                }
            }
        };

        let footer = rank.map(|rank| {
            Footer::new(format!(
                "The current rank for {pp}pp is #{rank}",
                pp = with_comma_float(goal_pp),
                rank = with_comma_int(rank),
            ))
        });

        Self {
            author: author!(user),
            description,
            footer,
            thumbnail: user.avatar_url,
            title,
        }
    }
}

impl EmbedData for PPMissingEmbed {
    fn into_builder(self) -> EmbedBuilder {
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
