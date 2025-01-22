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
    pub(super) async fn new() -> Self {
        let mut modes_count = [0; 4];
        let mut total = 0;
        let mut channels = HashSet::with_hasher(IntHasher);

        let users = OsuTracking::users().pin_owned();
        let unique_users = users.len();

        for (_, entry) in users.iter() {
            const MODES: [GameMode; 4] = [
                GameMode::Osu,
                GameMode::Taiko,
                GameMode::Catch,
                GameMode::Mania,
            ];

            for mode in MODES {
                let user = entry.get_unchecked(mode);
                let channels_guard = user.channels().await;

                if channels_guard.is_empty() {
                    continue;
                }

                total += 1;
                modes_count[mode as usize] += 1;

                channels.extend(channels_guard.keys().copied());
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
