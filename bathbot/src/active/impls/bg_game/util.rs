use std::collections::VecDeque;

use bathbot_psql::model::games::{DbMapTagEntry, MapsetTagsEntries};

pub fn get_random_mapset<'m>(
    entries: &'m MapsetTagsEntries,
    previous_ids: &mut VecDeque<i32>,
) -> &'m DbMapTagEntry {
    let buffer_size = entries.tags.len() / 2;

    loop {
        let random_index = rand::random::<usize>() % entries.tags.len();
        let entry = &entries.tags[random_index];

        if !previous_ids.contains(&entry.mapset_id) {
            previous_ids.push_front(entry.mapset_id);

            if previous_ids.len() > buffer_size {
                previous_ids.pop_back();
            }

            return entry;
        }
    }
}
