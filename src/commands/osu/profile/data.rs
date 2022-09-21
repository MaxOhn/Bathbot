use std::{
    borrow::Cow,
    cmp::{Ordering::Equal, PartialOrd},
    collections::BTreeMap,
};

use hashbrown::HashMap;
use rosu_v2::prelude::{GameMode, GameMods, Score, User, UserStatistics};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::osu::MinMaxAvg,
    util::{hasher::IntHasher, osu::BonusPP},
};

use super::ProfileEmbedMap;

pub struct ProfileData {
    pub user: User,
    pub scores: Vec<Score>,
    pub embeds: ProfileEmbedMap,
    pub discord_id: Option<Id<UserMarker>>,
    pub profile_result: Option<ProfileResult>,
    pub globals_count: Option<BTreeMap<usize, Cow<'static, str>>>,
}

impl ProfileData {
    pub(super) fn new(user: User, scores: Vec<Score>, discord_id: Option<Id<UserMarker>>) -> Self {
        Self {
            user,
            scores,
            embeds: ProfileEmbedMap::default(),
            discord_id,
            profile_result: None,
            globals_count: None,
        }
    }

    /// Check how many of a user's top scores are on their own maps
    pub fn own_top_scores(&self) -> usize {
        let ranked_maps_count =
            self.user.ranked_mapset_count.unwrap_or(0) + self.user.guest_mapset_count.unwrap_or(0);

        if ranked_maps_count > 0 {
            self.scores
                .iter()
                .filter(|score| {
                    score
                        .map
                        .as_ref()
                        .map(|map| map.creator_id == self.user.user_id)
                        .unwrap_or(false)
                })
                .count()
        } else {
            0
        }
    }
}

pub struct ProfileResult {
    pub mode: GameMode,

    pub acc: MinMaxAvg<f32>,
    pub pp: MinMaxAvg<f32>,
    pub bonus_pp: f32,
    pub map_combo: u32,
    pub combo: MinMaxAvg<u32>,
    pub map_len: MinMaxAvg<u32>,

    pub mappers: Vec<(u32, usize, f32)>,
    pub mod_combs_count: Option<Vec<(GameMods, u32)>>,
    pub mod_combs_pp: Vec<(GameMods, f32)>,
    pub mods_count: Vec<(GameMods, u32)>,
}

impl ProfileResult {
    pub(super) fn calc(mode: GameMode, scores: &[Score], stats: &UserStatistics) -> Self {
        let mut acc = MinMaxAvg::new();
        let mut pp = MinMaxAvg::new();
        let mut combo = MinMaxAvg::new();
        let mut map_len = MinMaxAvg::new();
        let mut map_combo = 0;
        let mut mapper_count = HashMap::with_capacity_and_hasher(10, IntHasher);
        let len = scores.len() as f32;
        let mut mod_combs = HashMap::with_capacity(5);
        let mut mods = HashMap::with_capacity(5);
        let mut mult_mods = false;
        let mut bonus_pp = BonusPP::new();

        for (i, score) in scores.iter().enumerate() {
            let map = score.map.as_ref().unwrap();

            acc.add(score.accuracy);

            if let Some(score_pp) = score.pp {
                pp.add(score_pp);
            }

            if let Some(weighted_pp) = score.weight.map(|w| w.pp) {
                bonus_pp.update(weighted_pp, i);

                let (count, pp) = mapper_count.entry(map.creator_id).or_insert((0, 0.0));
                *count += 1;
                *pp += weighted_pp;

                let mut mod_comb = mod_combs.entry(score.mods).or_insert((0, 0.0));
                mod_comb.0 += 1;
                mod_comb.1 += weighted_pp;
            }

            combo.add(score.max_combo);

            if let Some(combo) = map.max_combo {
                map_combo += combo;
            }

            let seconds_drain = if score.mods.contains(GameMods::DoubleTime) {
                map.seconds_drain as f32 / 1.5
            } else if score.mods.contains(GameMods::HalfTime) {
                map.seconds_drain as f32 * 1.5
            } else {
                map.seconds_drain as f32
            };

            map_len.add(seconds_drain);

            if score.mods.is_empty() {
                *mods.entry(GameMods::NoMod).or_insert(0) += 1;
            } else {
                mult_mods |= score.mods.len() > 1;

                for m in score.mods {
                    *mods.entry(m).or_insert(0) += 1;
                }
            }
        }

        map_combo /= len as u32;

        mod_combs
            .values_mut()
            .for_each(|(count, _)| *count = (*count as f32 * 100.0 / len) as u32);

        mods.values_mut()
            .for_each(|count| *count = (*count as f32 * 100.0 / len) as u32);

        let mut mappers: Vec<_> = mapper_count
            .into_iter()
            .map(|(id, (count, pp))| (id, count, pp))
            .collect();

        mappers.sort_unstable_by(|(_, count_a, pp_a), (_, count_b, pp_b)| {
            count_b
                .cmp(count_a)
                .then_with(|| pp_b.partial_cmp(pp_a).unwrap_or(Equal))
        });

        mappers.truncate(5);

        let mod_combs_count = if mult_mods {
            let mut mod_combs_count: Vec<_> = mod_combs
                .iter()
                .map(|(name, (count, _))| (*name, *count))
                .collect();

            mod_combs_count.sort_unstable_by(|(_, a), (_, b)| b.cmp(a));

            Some(mod_combs_count)
        } else {
            None
        };

        let mod_combs_pp = {
            let mut mod_combs_pp: Vec<_> = mod_combs
                .into_iter()
                .map(|(name, (_, avg))| (name, avg))
                .collect();

            mod_combs_pp.sort_unstable_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(Equal));

            mod_combs_pp
        };

        let mut mods_count: Vec<_> = mods.into_iter().collect();
        mods_count.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        Self {
            mode,
            acc,
            pp,
            bonus_pp: bonus_pp.calculate(stats),
            combo,
            map_combo,
            map_len: map_len.into(),
            mappers,
            mod_combs_count,
            mod_combs_pp,
            mods_count,
        }
    }
}
