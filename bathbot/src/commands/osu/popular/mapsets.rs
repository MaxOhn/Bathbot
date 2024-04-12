use std::{alloc, collections::HashMap, mem, sync::Arc};

use bathbot_model::OsuTrackerMapsetEntry;
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSUTRACKER_ISSUE},
    IntHasher,
};
use eyre::{Report, Result};
use rkyv::{DeserializeUnsized, Infallible};
use rosu_v2::prelude::Username;
use time::OffsetDateTime;

use crate::{
    active::{impls::PopularMapsetsPagination, ActiveMessages},
    core::{Context, ContextExt},
    manager::redis::RedisData,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

const COUNTS_LEN: usize = 727;

pub(super) async fn mapsets(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let counts: Vec<OsuTrackerMapsetEntry> = match ctx.redis().osutracker_stats().await {
        Ok(RedisData::Original(mut stats)) => {
            stats.mapset_count.truncate(COUNTS_LEN);

            stats.mapset_count
        }
        Ok(RedisData::Archive(stats)) => {
            let counts = &stats.mapset_count;
            let slice = &counts[..counts.len().min(COUNTS_LEN)];

            unsafe {
                // Deserialize to some location and get a pointer to it as *const ()
                // i.e. a thin 8 byte pointer
                let ptr =
                    <[_] as DeserializeUnsized<[OsuTrackerMapsetEntry], _>>::deserialize_unsized(
                        slice,
                        &mut Infallible,
                        |layout| alloc::alloc(layout),
                    )
                    .unwrap();

                // Transmute into a wide 16 byte pointer by appending the slice's metadata
                // i.e. its length
                let ptr = mem::transmute::<_, *mut [_]>((ptr, slice.len()));

                // Construct a vec from the pointer
                Box::<[_]>::from_raw(ptr).into()
            }
        }
        Err(err) => {
            let _ = command.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.wrap_err("failed to get cached osutracker stats"));
        }
    };

    let mut mapsets = HashMap::with_hasher(IntHasher);

    for entry in counts.iter().take(10) {
        let mapset_id = entry.mapset_id;

        let mapset = match ctx.osu_map().mapset(mapset_id).await {
            Ok(mapset) => mapset,
            Err(err) => {
                let _ = command.error(&ctx, GENERAL_ISSUE).await;

                return Err(Report::new(err));
            }
        };

        let entry = MapsetEntry {
            creator: mapset.creator.into(),
            name: format!("{} - {}", mapset.artist, mapset.title),
            mapset_id,
            ranked_date: mapset.ranked_date.unwrap_or_else(OffsetDateTime::now_utc),
            user_id: mapset.user_id as u32,
        };

        mapsets.insert(mapset_id, entry);
    }

    let pagination = PopularMapsetsPagination::builder()
        .entries(counts.into_boxed_slice())
        .mapsets(mapsets)
        .msg_owner(command.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, &mut command)
        .await
}

pub struct MapsetEntry {
    pub creator: Username,
    pub name: String,
    pub mapset_id: u32,
    pub ranked_date: OffsetDateTime,
    pub user_id: u32,
}
