use std::{alloc, mem, sync::Arc};

use eyre::Result;
use rkyv::{DeserializeUnsized, Infallible};

use crate::{
    core::Context,
    custom_client::OsuTrackerMapperEntry,
    pagination::OsuTrackerMappersPagination,
    util::{constants::OSUTRACKER_ISSUE, interaction::InteractionCommand, InteractionCommandExt},
};

pub(super) async fn mappers(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    const LIMIT: usize = 500;

    let counts: Vec<OsuTrackerMapperEntry> = match ctx.redis().osutracker_stats().await {
        Ok(stats) => {
            let counts = &stats.get().mapper_count;
            let slice = &counts[..counts.len().min(LIMIT)];

            unsafe {
                // Deserialize to some location and get a pointer to it as *const ()
                // i.e. a thin 8 byte pointer
                let ptr =
                    <[_] as DeserializeUnsized<[OsuTrackerMapperEntry], _>>::deserialize_unsized(
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

    OsuTrackerMappersPagination::builder(counts)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}
