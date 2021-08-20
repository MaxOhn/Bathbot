use crate::{
    embeds::Author,
    util::{
        constants::MAP_THUMB_URL,
        numbers::{round, with_comma_float},
    },
};

use rosu_v2::prelude::{Beatmap, GameMods, RankStatus, Score, User};
use std::fmt::Write;

pub struct FixScoreEmbed {
    author: Author,
    description: String,
    thumbnail: String,
    title: String,
    url: String,
}

impl FixScoreEmbed {
    pub fn new(
        user: User,
        map: Beatmap,
        scores: Option<(Box<Score>, Vec<Score>)>,
        unchoked_pp: Option<f32>,
        mods: Option<GameMods>,
    ) -> Self {
        let author = author!(user);
        let url = map.url;
        let thumbnail = format!("{}{}l.jpg", MAP_THUMB_URL, map.mapset_id);

        let mapset = map.mapset.as_ref().unwrap();
        let title = format!("{} - {} [{}]", mapset.artist, mapset.title, map.version);

        // The score can be unchoked
        let description = if let Some(pp) = unchoked_pp {
            let (score, mut best) = scores.unwrap();

            let mut description = format!(
                "An FC would have improved the score from {} to **{}pp**. ",
                round(score.pp.unwrap_or(0.0)),
                round(pp),
            );

            let in_best = best.iter().any(|s| s.pp.unwrap_or(0.0) < pp);

            // Map is ranked
            let _ = if matches!(map.status, RankStatus::Ranked | RankStatus::Approved) {
                if in_best || best.len() < 100 {
                    let mut old_idx = None;
                    let mut actual_offset = 0.0;

                    if let Some(idx) = best.iter().position(|s| s == &*score) {
                        actual_offset = best.remove(idx).weight.unwrap().pp;
                        old_idx.replace(idx + 1);
                    }

                    let (new_idx, new_pp) = new_pp(pp, &user, &best, actual_offset);

                    if let Some(old_idx) = old_idx {
                        write!(
                            description,
                            "The score would have moved from personal #{} to #{}, \
                            pushing the total pp to **{}pp**.",
                            old_idx,
                            new_idx,
                            with_comma_float(new_pp)
                        )
                    } else {
                        write!(
                            description,
                            "It would have been a personal top #{}, \
                            pushing the total pp to **{}pp**.",
                            new_idx,
                            with_comma_float(new_pp),
                        )
                    }
                } else {
                    let lowest_pp_required = best.last().and_then(|score| score.pp).unwrap_or(0.0);

                    write!(
                        description,
                        "A new top100 score requires {}pp.",
                        lowest_pp_required
                    )
                }
            // Map not ranked but in top100
            } else if in_best || best.len() < 100 {
                let (idx, new_pp) = new_pp(pp, &user, &best, 0.0);

                write!(
                    description,
                    "If the map wasn't {:?}, an FC would have \
                    been a personal #{}, pushing the total pp to **{}pp**.",
                    map.status,
                    idx,
                    with_comma_float(new_pp)
                )
            // Map not ranked and not in top100
            } else {
                let lowest_pp_required = best.last().and_then(|score| score.pp).unwrap_or(0.0);

                write!(
                    description,
                    "A top100 score requires {}pp but the map is {:?} anyway.",
                    lowest_pp_required, map.status
                )
            };

            description
        // The score is already an FC
        } else if let Some((score, best)) = scores {
            let mut description = format!("Already got a {}pp FC", round(score.pp.unwrap_or(0.0)));

            // Map is not ranked
            if !matches!(map.status, RankStatus::Ranked | RankStatus::Approved) {
                if best.iter().any(|s| s.pp < score.pp) || best.len() < 100 {
                    let (idx, new_pp) = new_pp(score.pp.unwrap_or(0.0), &user, &best, 0.0);

                    let _ = write!(
                        description,
                        ". If the map wasn't {:?} the score would have \
                        been a personal #{}, pushing the total pp to **{}pp**.",
                        map.status,
                        idx,
                        with_comma_float(new_pp)
                    );
                } else {
                    let lowest_pp_required = best.last().and_then(|score| score.pp).unwrap_or(0.0);

                    let _ = write!(
                        description,
                        ". A top100 score would have required {}pp but the map is {:?} anyway.",
                        lowest_pp_required, map.status
                    );
                }
            }

            description
        // The user has no score on the map
        } else {
            match mods {
                Some(mods) => format!("No {} score on the map", mods),
                None => "No score on the map".to_owned(),
            }
        };

        Self {
            author,
            description,
            thumbnail,
            title,
            url,
        }
    }
}

impl_builder!(FixScoreEmbed {
    author,
    description,
    thumbnail,
    title,
    url,
});

fn new_pp(pp: f32, user: &User, scores: &[Score], actual_offset: f32) -> (usize, f32) {
    let actual: f32 = scores
        .iter()
        .filter_map(|s| s.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - (actual + actual_offset);
    let mut new_pp = 0.0;
    let mut used = false;
    let mut new_pos = scores.len();
    let mut factor = 1.0;

    let pp_iter = scores.iter().take(99).filter_map(|s| s.pp).enumerate();

    for (i, pp_value) in pp_iter {
        if !used && pp_value < pp {
            used = true;
            new_pp += pp * factor;
            factor *= 0.95;
            new_pos = i + 1;
        }

        new_pp += pp_value * factor;
        factor *= 0.95;
    }

    if !used {
        new_pp += pp * factor;
    };

    (new_pos, new_pp + bonus_pp)
}
