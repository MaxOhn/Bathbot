use std::{cmp::Ordering, convert::identity, fmt::Write};

use bathbot_macros::EmbedData;
use bathbot_util::{
    constants::OSU_BASE,
    numbers::{round, WithComma},
    osu::{ExtractablePp, PpListUtil},
    AuthorBuilder, CowUtils,
};
use rosu_v2::prelude::{GameModsIntermode, RankStatus, Score};
use time::OffsetDateTime;

use crate::{
    commands::osu::{FixEntry, FixScore},
    manager::redis::osu::CachedUser,
    util::CachedUserExt,
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
    pub fn new(entry: &FixEntry, mods: Option<GameModsIntermode>) -> Self {
        let FixEntry { user, map, score } = entry;

        let author = user.author_builder(false);
        let url = format!("{OSU_BASE}b/{}", map.map_id());
        let thumbnail = map.thumbnail().to_owned();

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
                    "A {mods} FC would have improved the score from {from} to **{to}pp**. ",
                    mods = fix_score.score.mods,
                    from = round(score.pp),
                    to = round(if_fc.pp),
                );

                let in_top = top.iter().any(|s| s.pp.unwrap_or(0.0) < if_fc.pp);

                // Map is ranked
                let _ = if matches!(map.status(), RankStatus::Ranked | RankStatus::Approved) {
                    if in_top || top.len() < 100 {
                        let NewPp {
                            old_pos,
                            new_pos,
                            new_total,
                        } = NewPp::new(if_fc.pp, user, top, score.ended_at);

                        if let Some(old_pos) = old_pos {
                            write!(
                                description,
                                "The score would have moved from personal #{old_pos} to #{new_pos}, \
                                pushing their total pp to **{}pp**.",
                                WithComma::new(new_total),
                            )
                        } else {
                            write!(
                                description,
                                "It would have been a personal top #{new_pos}, \
                                pushing their total pp to **{}pp**.",
                                WithComma::new(new_total),
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
                    let NewPp {
                        old_pos: _,
                        new_pos,
                        new_total,
                    } = NewPp::new(if_fc.pp, user, top, score.ended_at);

                    write!(
                        description,
                        "If the map wasn't {status:?}, an FC would have \
                        been a personal #{new_pos}, pushing their total pp to **{pp}pp**.",
                        status = map.status(),
                        pp = WithComma::new(new_total),
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
                format!(
                    "Already got a {pp}pp {mods} FC",
                    pp = round(score.pp),
                    mods = fix_score.score.mods
                )
            }
        } else if let Some(mods) = mods {
            format!("No {mods} score on the map")
        } else {
            "No score on the map".to_owned()
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

struct NewPp {
    old_pos: Option<usize>,
    new_pos: usize,
    new_total: f32,
}

impl NewPp {
    fn new(pp: f32, user: &CachedUser, scores: &[Score], datetime: OffsetDateTime) -> NewPp {
        let datetime = datetime.unix_timestamp();

        let old_idx = scores
            .iter()
            .position(|score| (score.ended_at.unix_timestamp() - datetime).abs() <= 2);

        let mut extracted_pp = scores.extract_pp();
        let old_weighted = extracted_pp.accum_weighted();

        if let Some(old_idx) = old_idx {
            extracted_pp.remove(old_idx);
        }

        let new_idx = extracted_pp
            .binary_search_by(|n| pp.partial_cmp(n).unwrap_or(Ordering::Equal))
            .unwrap_or_else(identity);

        if new_idx == extracted_pp.len() {
            extracted_pp.push(pp);
        } else {
            extracted_pp.insert(new_idx, pp);
        }

        let new_weighted = extracted_pp.accum_weighted();
        let total = user
            .statistics
            .as_ref()
            .expect("missing stats")
            .pp
            .to_native();
        let new_total = total - old_weighted + new_weighted;

        NewPp {
            old_pos: old_idx.map(|i| i + 1),
            new_pos: new_idx + 1,
            new_total,
        }
    }
}
