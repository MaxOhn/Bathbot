use std::collections::VecDeque;

use rand::Rng;

use crate::database::MapsetTagWrapper;

#[allow(clippy::needless_lifetimes)]
pub fn get_random_mapset<'m>(
    mapsets: &'m [MapsetTagWrapper],
    previous_ids: &mut VecDeque<u32>,
) -> &'m MapsetTagWrapper {
    let mut rng = rand::thread_rng();
    let buffer_size = mapsets.len() / 2;

    loop {
        let random_index = rng.gen::<usize>() % mapsets.len();
        let mapset = &mapsets[random_index];

        if !previous_ids.contains(&mapset.mapset_id) {
            previous_ids.push_front(mapset.mapset_id);

            if previous_ids.len() > buffer_size {
                previous_ids.pop_back();
            }

            return mapset;
        }
    }
}
