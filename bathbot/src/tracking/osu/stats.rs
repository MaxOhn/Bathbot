use std::collections::HashSet;

use bathbot_util::IntHasher;
use rosu_v2::prelude::GameMode;

use crate::tracking::OsuTracking;

pub struct OsuTrackingStats {
    pub total: usize,
    pub unique_users: usize,
    pub count_osu: usize,
    pub count_taiko: usize,
    pub count_catch: usize,
    pub count_mania: usize,
    pub channels: usize,
}

impl OsuTrackingStats {
    pub(super) fn new() -> Self {
        let mut modes_count = [0; 4];
        let mut total = 0;
        let mut channels = HashSet::with_hasher(IntHasher);

        let users = OsuTracking::users().pin();
        let unique_users = users.len();

        for (_, entry) in users.iter() {
            const MODES: [GameMode; 4] = [
                GameMode::Osu,
                GameMode::Taiko,
                GameMode::Catch,
                GameMode::Mania,
            ];

            for mode in MODES {
                let Some(user) = entry.get(mode) else {
                    continue;
                };

                total += 1;
                modes_count[mode as usize] += 1;

                let channels_guard = user.guard_channels();
                let iter = user
                    .iter_channels(&channels_guard)
                    .map(|(channel_id, _)| *channel_id);

                channels.extend(iter);
            }
        }

        Self {
            total,
            unique_users,
            count_osu: modes_count[GameMode::Osu as usize],
            count_taiko: modes_count[GameMode::Taiko as usize],
            count_catch: modes_count[GameMode::Catch as usize],
            count_mania: modes_count[GameMode::Mania as usize],
            channels: channels.len(),
        }
    }
}
