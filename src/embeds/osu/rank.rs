use std::{cmp::Ordering, iter};

use crate::{
    commands::osu::RankData,
    embeds::Author,
    util::{
        numbers::{with_comma_float, with_comma_int},
        osu::pp_missing,
    },
};

use rosu_v2::model::score::Score;

pub struct RankEmbed {
    description: String,
    title: String,
    thumbnail: String,
    author: Author,
}

impl RankEmbed {
    pub fn new(data: RankData, scores: Option<Vec<Score>>, each: Option<f32>) -> Self {
        let (title, description) = match &data {
            RankData::Sub10k {
                user,
                rank,
                country,
                rank_holder,
            } => {
                let user_pp = user.statistics.as_ref().unwrap().pp;
                let rank_holder_pp = rank_holder.statistics.as_ref().unwrap().pp;

                let country = country.as_ref().map(|code| code.as_str()).unwrap_or("#");

                let title = format!(
                    "How many pp is {name} missing to reach rank {country}{rank}?",
                    name = user.username,
                );

                let description = if user.user_id == rank_holder.user_id {
                    format!("{} is already at rank #{rank}.", user.username)
                } else if user_pp > rank_holder_pp {
                    format!(
                        "Rank {country}{rank} is currently held by {holder_name} with \
                        **{holder_pp}pp**, so {name} is already above that with **{pp}pp**.",
                        holder_name = rank_holder.username,
                        holder_pp = with_comma_float(rank_holder_pp),
                        name = user.username,
                        pp = with_comma_float(user_pp)
                    )
                } else if let Some(mut scores) = scores {
                    match (scores.last().and_then(|s| s.pp), each) {
                        (Some(last_pp), Some(each)) if each < last_pp => {
                            format!(
                                "Rank {country}{rank} is currently held by {holder_name} with \
                                **{holder_pp}pp**, so {name} is missing **{missing}** raw pp.\n\
                                A new top100 score requires at least **{last_pp}pp** \
                                so {holder_pp} total pp can't be reached with {each}pp scores.",
                                holder_name = rank_holder.username,
                                holder_pp = with_comma_float(rank_holder_pp),
                                name = user.username,
                                missing = with_comma_float(rank_holder_pp - user_pp),
                                last_pp = with_comma_float(last_pp),
                                each = with_comma_float(each),
                            )
                        }
                        (_, Some(each)) => {
                            let (required, idx) =
                                pp_missing(user_pp, rank_holder_pp, scores.as_slice());

                            if required < each {
                                format!(
                                    "Rank {country}{rank} is currently held by {holder_name} with \
                                    **{holder_pp}pp**, so {name} is missing **{missing}** raw pp.\n\
                                    To reach {holder_pp}pp with one additional score, {name} needs to \
                                    perform a **{required}pp** score which would be the top #{idx}",
                                    holder_name = rank_holder.username,
                                    holder_pp = with_comma_float(rank_holder_pp),
                                    name = user.username,
                                    missing = with_comma_float(rank_holder_pp - user_pp),
                                    required = with_comma_float(required),
                                    idx = idx + 1,
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

                                let bonus_pp = user_pp - (top + bot);
                                top += bonus_pp;
                                let len = scores.len();

                                let mut n_each = len;

                                for i in idx..len {
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

                                    if top + factor * each + bot >= rank_holder_pp {
                                        // requires n_each many new scores of `each` many pp and one additional score
                                        n_each = i - idx;
                                        break;
                                    }

                                    top += factor * each;
                                }

                                if n_each == len {
                                    format!(
                                        "Rank {country}{rank} is currently held by {holder_name} with \
                                        **{holder_pp}pp**, so {name} is missing **{missing}** raw pp.\n\
                                        Filling up {name}'{genitiv} top100 with {amount} new \
                                        {each}pp score{plural} would only lead to **{top}pp** which \
                                        is still less than {holder_pp}pp.",
                                        holder_name = rank_holder.username,
                                        holder_pp = with_comma_float(rank_holder_pp),
                                        amount = len - idx,
                                        each = with_comma_float(each),
                                        missing = with_comma_float(rank_holder_pp - user_pp),
                                        plural = if len - idx != 1 { "s" } else { "" },
                                        genitiv = if idx != 1 { "s" } else { "" },
                                        top = with_comma_float(top),
                                        name = user.username,
                                    )
                                } else {
                                    let mut pps: Vec<_> = scores
                                        .iter()
                                        .filter_map(|s| s.pp)
                                        .chain(iter::repeat(each).take(n_each))
                                        .collect();

                                    pps.sort_unstable_by(|a, b| {
                                        b.partial_cmp(a).unwrap_or(Ordering::Equal)
                                    });

                                    let accum = pps.iter().enumerate().fold(0.0, |sum, (i, pp)| {
                                        sum + pp * 0.95_f32.powi(i as i32)
                                    });

                                    // Calculate the pp of the missing score after adding `n_each` many `each` pp scores
                                    let total = accum + bonus_pp;
                                    let (required, _) =
                                        pp_missing(total, rank_holder_pp, pps.as_slice());

                                    format!(
                                        "Rank {country}{rank} is currently held by {holder_name} with \
                                        **{holder_pp}pp**, so {name} is missing **{missing}** raw pp.\n\
                                        To reach {holder_pp}pp, {name} needs to perform **{n_each}** \
                                        more {each}pp score{plural} and one **{required}pp** score.",
                                        holder_name = rank_holder.username,
                                        holder_pp = with_comma_float(rank_holder_pp),
                                        missing = with_comma_float(rank_holder_pp - user_pp),
                                        each = with_comma_float(each),
                                        plural = if n_each != 1 { "s" } else { "" },
                                        name = user.username,
                                        required = with_comma_float(required),
                                    )
                                }
                            }
                        }
                        _ => {
                            let (required, _) =
                                pp_missing(user_pp, rank_holder_pp, scores.as_slice());

                            format!(
                                "Rank {country}{rank} is currently held by {holder_name} with \
                                **{holder_pp}pp**, so {name} is missing **{missing}** raw pp, \
                                achievable with a single score worth **{pp}pp**.",
                                holder_name = rank_holder.username,
                                holder_pp = with_comma_float(rank_holder_pp),
                                name = user.username,
                                missing = with_comma_float(rank_holder_pp - user_pp),
                                pp = with_comma_float(required),
                            )
                        }
                    }
                } else {
                    format!(
                        "Rank {country}{rank} is currently held by {holder_name} with \
                        **{holder_pp}pp**, so {name} is missing **{holder_pp}** raw pp, \
                        achievable with a single score worth **{holder_pp}pp**.",
                        holder_name = rank_holder.username,
                        holder_pp = with_comma_float(rank_holder_pp),
                        name = user.username,
                    )
                };

                (title, description)
            }
            RankData::Over10k {
                user,
                rank,
                required_pp,
            } => {
                let user_pp = user.statistics.as_ref().unwrap().pp;

                let title = format!(
                    "How many pp is {name} missing to reach rank #{rank}?",
                    name = user.username,
                    rank = with_comma_int(*rank),
                );

                let description = if user_pp > *required_pp {
                    format!(
                        "Rank #{rank} currently requires **{required_pp}pp**, \
                        so {name} is already above that with **{pp}pp**.",
                        rank = with_comma_int(*rank),
                        required_pp = with_comma_float(*required_pp),
                        name = user.username,
                        pp = with_comma_float(user_pp)
                    )
                } else if let Some(mut scores) = scores {
                    match (scores.last().and_then(|s| s.pp), each) {
                        (Some(last_pp), Some(each)) if each < last_pp => {
                            format!(
                                "Rank #{rank} currently requires **{required_pp}pp**, \
                                so {name} is missing **{missing}** raw pp.\n\
                                A new top100 score requires at least **{last_pp}pp** \
                                so {required_pp} total pp can't be reached with {each}pp scores.",
                                required_pp = with_comma_float(*required_pp),
                                name = user.username,
                                missing = with_comma_float(required_pp - user_pp),
                                last_pp = with_comma_float(last_pp),
                                each = with_comma_float(each),
                            )
                        }
                        (_, Some(each)) => {
                            let (required, idx) =
                                pp_missing(user_pp, *required_pp, scores.as_slice());

                            if required < each {
                                format!(
                                    "Rank #{rank} currently requires **{required_pp}pp**, \
                                    so {name} is missing **{missing}** raw pp.\n\
                                    To reach {required_pp}pp with one additional score, {name} needs to \
                                    perform a **{required}pp** score which would be the top #{idx}",
                                    name = user.username,
                                    required_pp = with_comma_float(*required_pp),
                                    missing = with_comma_float(required_pp - user_pp),
                                    required = with_comma_float(required),
                                    idx = idx + 1,
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

                                let bonus_pp = user_pp - (top + bot);
                                top += bonus_pp;
                                let len = scores.len();

                                let mut n_each = len;

                                for i in idx..len {
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

                                    if top + factor * each + bot >= *required_pp {
                                        // requires n_each many new scores of `each` many pp and one additional score
                                        n_each = i - idx;
                                        break;
                                    }

                                    top += factor * each;
                                }

                                if n_each == len {
                                    format!(
                                        "Rank #{rank} currently requires **{required_pp}pp**, \
                                        so {name} is missing **{missing}** raw pp.\n\
                                        Filling up {name}'{genitiv} top100 with {amount} new \
                                        {each}pp score{plural} would only lead to **{top}pp** which \
                                        is still less than {required_pp}pp.",
                                        required_pp = with_comma_float(*required_pp),
                                        amount = len - idx,
                                        each = with_comma_float(each),
                                        missing = with_comma_float(required_pp - user_pp),
                                        plural = if len - idx != 1 { "s" } else { "" },
                                        genitiv = if idx != 1 { "s" } else { "" },
                                        top = with_comma_float(top),
                                        name = user.username,
                                    )
                                } else {
                                    let mut pps: Vec<_> = scores
                                        .iter()
                                        .filter_map(|s| s.pp)
                                        .chain(iter::repeat(each).take(n_each))
                                        .collect();

                                    pps.sort_unstable_by(|a, b| {
                                        b.partial_cmp(a).unwrap_or(Ordering::Equal)
                                    });

                                    let accum = pps.iter().enumerate().fold(0.0, |sum, (i, pp)| {
                                        sum + pp * 0.95_f32.powi(i as i32)
                                    });

                                    // Calculate the pp of the missing score after adding `n_each` many `each` pp scores
                                    let total = accum + bonus_pp;
                                    let (required, _) =
                                        pp_missing(total, *required_pp, pps.as_slice());

                                    format!(
                                        "Rank #{rank} currently requires **{required_pp}pp**, \
                                        so {name} is missing **{missing}** raw pp.\n\
                                        To reach {required_pp}pp, {name} needs to perform **{n_each}** \
                                        more {each}pp score{plural} and one **{required}pp** score.",
                                        required_pp = with_comma_float(*required_pp),
                                        missing = with_comma_float(required_pp - user_pp),
                                        each = with_comma_float(each),
                                        plural = if n_each != 1 { "s" } else { "" },
                                        name = user.username,
                                        required = with_comma_float(required),
                                    )
                                }
                            }
                        }
                        _ => {
                            let (required, _) =
                                pp_missing(user_pp, *required_pp, scores.as_slice());

                            format!(
                                "Rank #{rank} currently requires **{required_pp}pp**, \
                                so {name} is missing **{missing}** raw pp, \
                                achievable with a single score worth **{pp}pp**.",
                                rank = with_comma_int(*rank),
                                required_pp = with_comma_float(*required_pp),
                                name = user.username,
                                missing = with_comma_float(required_pp - user_pp),
                                pp = with_comma_float(required),
                            )
                        }
                    }
                } else {
                    format!(
                        "Rank #{rank} currently requires **{required_pp}pp**, \
                        so {name} is missing **{required_pp}** raw pp, \
                        achievable with a single score worth **{required_pp}pp**.",
                        rank = with_comma_int(*rank),
                        required_pp = with_comma_float(*required_pp),
                        name = user.username,
                    )
                };

                (title, description)
            }
        };

        let user = data.user();

        Self {
            title,
            description,
            author: author!(user),
            thumbnail: user.avatar_url,
        }
    }
}

impl_builder!(RankEmbed {
    author,
    description,
    thumbnail,
    title,
});
