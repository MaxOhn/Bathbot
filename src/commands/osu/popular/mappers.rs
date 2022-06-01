use std::sync::Arc;

use rkyv::{Deserialize, Infallible};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    core::{commands::CommandOrigin, Context},
    custom_client::OsuTrackerMapperEntry,
    pagination::OsuTrackerMappersPagination,
    util::{constants::OSUTRACKER_ISSUE, ApplicationCommandExt},
    BotResult,
};

pub(super) async fn mappers(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    let mut counts: Vec<OsuTrackerMapperEntry> = match ctx.redis().osutracker_stats().await {
        Ok(stats) => stats
            .get()
            .mapper_count
            .deserialize(&mut Infallible)
            .unwrap(),
        Err(err) => {
            let _ = command.error(&ctx, OSUTRACKER_ISSUE).await;

            return Err(err.into());
        }
    };

    // TODO: only deserialize this many in the first place
    counts.truncate(500);

    OsuTrackerMappersPagination::builder(counts)
    .start_by_update()
        .start(ctx, CommandOrigin::Interaction { command })
        .await
}
