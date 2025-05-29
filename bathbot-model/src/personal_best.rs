use std::{cmp::Ordering, convert::identity, ops::Not};

use bathbot_util::MessageOrigin;
use rosu_v2::prelude::{RankStatus, Score};

use crate::ScoreSlim;

/// Note that all contained indices start at 0.
pub enum PersonalBestIndex {
    /// Found the score in the top100
    FoundScore { idx: usize },
    /// There was a score on the same map with more pp in the top100
    FoundBetter {
        #[allow(unused)]
        idx: usize,
    },
    /// Score is ranked and has enough pp to be in but wasn't found
    Presumably { idx: usize },
    /// Score is not ranked but has enough pp to be in the top100
    IfRanked { idx: usize },
    /// Score does not have enough pp to be in the top100
    NotTop100,
}

impl PersonalBestIndex {
    pub fn new(score: &ScoreSlim, map_id: u32, status: RankStatus, top100: &[Score]) -> Self {
        debug_assert!(top100.len() <= 100);

        // Note that the index is determined through float comparisons which
        // could result in issues
        let idx = top100
            .binary_search_by(|probe| {
                probe
                    .pp
                    .and_then(|pp| score.pp.partial_cmp(&pp))
                    .unwrap_or(Ordering::Less)
            })
            .unwrap_or_else(identity);

        if idx == 100 {
            return Self::NotTop100;
        } else if !matches!(status, RankStatus::Ranked | RankStatus::Approved)
            || top100[idx].ranked.is_some_and(bool::not)
        {
            return Self::IfRanked { idx };
        } else if let Some(top) = top100.get(idx) {
            if score.is_eq(top) {
                return Self::FoundScore { idx };
            } else if let Some((idx, _)) = top100
                .iter()
                .enumerate()
                .skip_while(|(_, top)| top.pp.is_none_or(|pp| pp < score.pp))
                .take_while(|(_, top)| top.pp.is_some_and(|pp| pp <= score.pp))
                .find(|(_, top)| score.is_eq(*top))
            {
                // If multiple scores have the exact same pp as the given score
                // then the initial `idx` might not correspond to it. Hence, if
                // the score at the initial `idx` does not match, we
                // double-check all scores with the same pp.
                return Self::FoundScore { idx };
            }
        }

        let better = &top100[..idx];

        // A case that's not covered is when there is a score with more pp on
        // the same map with the same mods that has less score than the current
        // score. This should only happen when the top scores haven't been
        // updated yet so the more-pp-but-less-score play is not yet replaced
        // with the new score. Fixes itself over time so it's probably fine to
        // ignore.
        if let Some(idx) = better.iter().position(|top| top.map_id == map_id) {
            Self::FoundBetter { idx }
        } else {
            Self::Presumably { idx }
        }
    }

    pub fn into_embed_description(self, origin: &MessageOrigin) -> Option<String> {
        match self {
            PersonalBestIndex::FoundScore { idx } => Some(format!("Personal Best #{}", idx + 1)),
            PersonalBestIndex::FoundBetter { .. } => None,
            PersonalBestIndex::Presumably { idx } => Some(format!(
                "Personal Best #{} [(?)]({origin} \
                \"the top100 did not include this score likely because the api \
                wasn't done processing but presumably the score is in there\")",
                idx + 1
            )),
            PersonalBestIndex::IfRanked { idx } => {
                Some(format!("Personal Best #{} (if ranked)", idx + 1))
            }
            PersonalBestIndex::NotTop100 => None,
        }
    }
}
