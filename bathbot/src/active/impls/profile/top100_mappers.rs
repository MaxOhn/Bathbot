use std::collections::HashMap;

use bathbot_util::IntHasher;
use rosu_v2::prelude::Username;

use super::{availability::MapperNames, ProfileMenu};

pub(super) struct Top100Mappers;

impl Top100Mappers {
    pub(super) async fn prepare(menu: &mut ProfileMenu) -> Option<Vec<MapperEntry<'_>>> {
        let mut entries: Vec<_> = {
            let user_id = menu.user.user_id.to_native();
            let mode = menu.user.mode;
            let scores = menu.scores.get(user_id, mode, menu.legacy_scores).await?;
            let mut entries = HashMap::with_capacity_and_hasher(32, IntHasher);

            for score in scores {
                if let Some(ref map) = score.map {
                    let (count, pp) = entries.entry(map.creator_id).or_insert((0, 0.0));

                    *count += 1;

                    if let Some(weight) = score.weight {
                        *pp += weight.pp;
                    }
                }
            }

            entries.into_iter().collect()
        };

        entries.sort_unstable_by(|(_, (count_a, pp_a)), (_, (count_b, pp_b))| {
            count_b.cmp(count_a).then_with(|| pp_b.total_cmp(pp_a))
        });

        entries.truncate(10);

        let MapperNames(mapper_names) = menu.mapper_names.get(&entries).await?;

        let mappers = entries
            .into_iter()
            .map(|(id, (count, pp))| MapperEntry {
                name: mapper_names
                    .get(&id)
                    .map_or("<unknown name>", Username::as_str),
                pp,
                count,
            })
            .collect();

        Some(mappers)
    }
}

pub(super) struct MapperEntry<'n> {
    pub name: &'n str,
    pub pp: f32,
    pub count: u8,
}
