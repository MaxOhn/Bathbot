use std::{cmp::Reverse, collections::HashMap};

use bathbot_util::IntHasher;
use rosu_v2::prelude::{GameMod, GameModIntermode, GameModsIntermode, Score};

use super::ProfileMenu;

pub(super) struct Top100Mods {
    pub percent_mods: Box<[(GameModIntermode, u8)]>,
    pub percent_mod_comps: Box<[(GameModsIntermode, u8)]>,
    pub pp_mod_comps: Box<[(GameModsIntermode, f32)]>,
}

impl Top100Mods {
    pub(super) async fn prepare(menu: &mut ProfileMenu) -> Option<Self> {
        let user_id = menu.user.user_id();
        let mode = menu.user.mode();

        menu.scores
            .get(user_id, mode, menu.legacy_scores)
            .await
            .map(Self::new)
    }

    fn new(scores: &[Score]) -> Self {
        let mut percent_mods = HashMap::with_hasher(IntHasher);
        let mut percent_mod_comps = HashMap::new();
        let mut pp_mod_comps = HashMap::<_, f32, _>::new();

        for score in scores {
            let mods: GameModsIntermode = score.mods.iter().map(GameMod::intermode).collect();

            if let Some(weight) = score.weight {
                *pp_mod_comps.entry(mods.clone()).or_default() += weight.pp;
            }

            *percent_mod_comps.entry(mods).or_default() += 1;

            for m in score.mods.iter().map(GameMod::intermode) {
                *percent_mods.entry(m).or_default() += 1;
            }
        }

        let mut percent_mods: Vec<_> = percent_mods.into_iter().collect();
        percent_mods.sort_unstable_by_key(|(_, percent)| Reverse(*percent));

        let mut percent_mod_comps: Vec<_> = percent_mod_comps.into_iter().collect();
        percent_mod_comps.sort_unstable_by_key(|(_, percent)| Reverse(*percent));

        let mut pp_mod_comps: Vec<_> = pp_mod_comps.into_iter().collect();
        pp_mod_comps.sort_unstable_by(|(_, a), (_, b)| b.total_cmp(a));

        Self {
            percent_mods: percent_mods.into_boxed_slice(),
            percent_mod_comps: percent_mod_comps.into_boxed_slice(),
            pp_mod_comps: pp_mod_comps.into_boxed_slice(),
        }
    }
}
