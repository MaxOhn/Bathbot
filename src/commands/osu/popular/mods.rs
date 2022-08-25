use std::sync::Arc;

use rkyv::{Deserialize, Infallible};

use crate::{
    core::Context,
    custom_client::OsuTrackerModsEntry,
    pagination::OsuTrackerModsPagination,
    util::{constants::OSUTRACKER_ISSUE, interaction::InteractionCommand, InteractionCommandExt},
    BotResult,
};

pub(super) async fn mods(ctx: Arc<Context>, mut command: InteractionCommand) -> BotResult<()> {
    let counts: Vec<OsuTrackerModsEntry> = match ctx.redis().osutracker_stats().await {
        Ok(stats) => stats
            .get()
            .user
            .mods_count
            .deserialize(&mut Infallible)
            .unwrap(),
        Err(err) => {
            let _ = command.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.into());
        }
    };

    OsuTrackerModsPagination::builder(counts)
        .start_by_update()
        .defer_components()
        .start(ctx, (&mut command).into())
        .await
}
