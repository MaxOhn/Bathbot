use std::{alloc, mem, sync::Arc};

use bathbot_model::OsuTrackerMapperEntry;
use bathbot_util::constants::OSUTRACKER_ISSUE;
use eyre::Result;
use rkyv::{DeserializeUnsized, Infallible};

use crate::{
    active::{impls::PopularMappersPagination, ActiveMessages},
    core::Context,
    manager::redis::RedisData,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

pub(super) async fn mappers(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    const LIMIT: usize = 500;

    let counts: Vec<OsuTrackerMapperEntry> = match ctx.redis().osutracker_stats().await {
        Ok(RedisData::Original(stats)) => {
            let mut counts = stats.mapper_count;
            counts.truncate(LIMIT);

            counts
        }
        Ok(RedisData::Archive(stats)) => {
            let counts = &stats.mapper_count;
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

    let pagination = PopularMappersPagination::builder()
        .entries(counts.into_boxed_slice())
        .msg_owner(command.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, &mut command)
        .await
}
