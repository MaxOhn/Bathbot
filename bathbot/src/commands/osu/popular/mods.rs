use std::sync::Arc;

use bathbot_util::constants::OSUTRACKER_ISSUE;
use eyre::Result;
use rkyv::{Deserialize, Infallible};

use crate::{
    active::{impls::PopularModsPagination, ActiveMessages},
    core::{Context, ContextExt},
    manager::redis::RedisData,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

pub(super) async fn mods(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let counts = match ctx.redis().osutracker_stats().await {
        Ok(RedisData::Original(stats)) => stats.user.mods_count,
        Ok(RedisData::Archive(stats)) => {
            stats.user.mods_count.deserialize(&mut Infallible).unwrap()
        }
        Err(err) => {
            let _ = command.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.wrap_err("Failed to get cached osutracker stats"));
        }
    };

    let pagination = PopularModsPagination::builder()
        .entries(counts)
        .msg_owner(command.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, &mut command)
        .await
}
