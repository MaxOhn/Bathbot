use std::sync::Arc;

use rkyv::{Deserialize, Infallible};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    core::{commands::CommandOrigin, Context},
    custom_client::OsuTrackerModsEntry,
    pagination::OsuTrackerModsPagination,
    util::{constants::OSUTRACKER_ISSUE, ApplicationCommandExt},
    BotResult,
};

pub(super) async fn mods(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
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
        .start(ctx, CommandOrigin::Interaction { command })
        .await
}
