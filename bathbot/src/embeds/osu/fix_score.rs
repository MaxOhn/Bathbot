use std::fmt::Write;

use bathbot_macros::EmbedData;
use rosu_v2::prelude::{GameMods, RankStatus, Score};

use crate::{
    commands::osu::{FixEntry, FixScore},
    manager::redis::{osu::User, RedisData},
    util::{
        builder::AuthorBuilder,
        constants::{MAP_THUMB_URL, OSU_BASE},
        numbers::{round, WithComma},
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct FixScoreEmbed {
    author: AuthorBuilder,
    description: String,
    thumbnail: String,
    title: String,
    url: String,
}

impl FixScoreEmbed {
    pub fn new(entry: &FixEntry, mods: Option<GameMods>) -> Self {
        let FixEntry { user, map, score } = entry;

        let author = user.author_builder();
        let url = format!("{OSU_BASE}b/{}", map.map_id());
        let thumbnail = format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id());

        let title = format!(
            "{} - {} [{}]",
            map.artist().cow_escape_markdown(),
            map.title().cow_escape_markdown(),
            map.version().cow_escape_markdown()
        );

        // The user has a score on the map
        let description = if let Some(fix_score) = score {
            let FixScore { score, top, if_fc } = fix_score;

            // The score can be unchoked
            if let Some(if_fc) = if_fc {
                let mut description = format!(
                    "An FC would have improved the score from {} to **{}pp**. ",
                    round(score.pp),
                    round(if_fc.pp),
                );

                let in_top = top.iter().any(|s| s.pp.unwrap_or(0.0) < if_fc.pp);

                // Map is ranked
                let _ = if matches!(map.status(), RankStatus::Ranked | RankStatus::Approved) {
                    if in_top || top.len() < 100 {
                        let mut old_idx = None;
                        let mut actual_offset = 0.0;

                        if let Some(idx) = top.iter().position(|s| {
                            (s.ended_at.unix_timestamp() - score.ended_at.unix_timestamp()).abs()
                                <= 2
                        }) {
                            actual_offset = top.get(idx).unwrap().weight.unwrap().pp;
                            old_idx = Some(idx + 1);
                        }

                        let (new_idx, new_pp) = new_pp(if_fc.pp, user, top, actual_offset);

                        if let Some(old_idx) = old_idx {
                            write!(
                                description,
                                "The score would have moved from personal #{old_idx} to #{new_idx}, \
                                pushing their total pp to **{}pp**.",
                                WithComma::new(new_pp),
                            )
                        } else {
                            write!(
                                description,
                                "It would have been a personal top #{new_idx}, \
                                pushing their total pp to **{}pp**.",
                                WithComma::new(new_pp),
                            )
                        }
                    } else {
                        let lowest_pp_required =
                            top.last().and_then(|score| score.pp).map_or(0.0, round);

                        write!(
                            description,
                            "A new top100 score requires {lowest_pp_required}pp."
                        )
                    }
                // Map not ranked but in top100
                } else if in_top || top.len() < 100 {
                    let (idx, new_pp) = new_pp(if_fc.pp, user, top, 0.0);

                    write!(
                        description,
                        "If the map wasn't {status:?}, an FC would have \
                        been a personal #{idx}, pushing their total pp to **{pp}pp**.",
                        status = map.status(),
                        pp = WithComma::new(new_pp),
                    )
                // Map not ranked and not in top100
                } else {
                    let lowest_pp_required =
                        top.last().and_then(|score| score.pp).map_or(0.0, round);

                    write!(
                        description,
                        "A top100 score requires {lowest_pp_required}pp but the map is {status:?} anyway.",
                        status = map.status(),
                    )
                };

                description
            } else {
                // The score is already an FC
                let mut description = format!("Already got a {}pp FC", round(score.pp));

                // Map is not ranked
                if !matches!(map.status(), RankStatus::Ranked | RankStatus::Approved) {
                    if top.iter().any(|s| s.pp < Some(score.pp)) || top.len() < 100 {
                        let (idx, new_pp) = new_pp(score.pp, user, top, 0.0);

                        let _ = write!(
                            description,
                            ". If the map wasn't {status:?} the score would have \
                            been a personal #{idx}, pushing their total pp to **{pp}pp**.",
                            status = map.status(),
                            pp = WithComma::new(new_pp),
                        );
                    } else {
                        let lowest_pp_required =
                            top.last().and_then(|score| score.pp).map_or(0.0, round);

                        let _ = write!(
                            description,
                            ". A top100 score would have required {lowest_pp_required}pp but the map is {status:?} anyway.",
                            status = map.status(),
                        );
                    }
                }

                description
            }
        } else {
            // The user has no score on the map
            match mods {
                Some(mods) => format!("No {mods} score on the map"),
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

fn new_pp(pp: f32, user: &RedisData<User>, scores: &[Score], actual_offset: f32) -> (usize, f32) {
    let actual: f32 = scores
        .iter()
        .filter_map(|s| s.weight)
        .fold(0.0, |sum, weight| sum + weight.pp);

    let total = user.peek_stats(|stats| stats.pp);
    let bonus_pp = total - (actual + actual_offset);
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
