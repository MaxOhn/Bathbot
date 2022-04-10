use std::sync::Arc;

use eyre::Report;
use rkyv::{Deserialize, Infallible};

use crate::{
    core::Context,
    custom_client::OsuTrackerModsEntry,
    embeds::EmbedData,
    embeds::OsuTrackerModsEmbed,
    pagination::{OsuTrackerModsPagination, Pagination},
    util::{constants::OSUTRACKER_ISSUE, numbers},
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

    let pages = numbers::div_euclid(20, counts.len());
    let initial = &counts[..counts.len().min(20)];

    let embed = OsuTrackerModsEmbed::new(initial, (1, pages)).into_builder();
    let builder = MessageBuilder::new().embed(embed.build());

    let response_raw = command.update(&ctx, &builder).await?;

    if counts.len() <= 20 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    let pagination = OsuTrackerModsPagination::new(response, counts);
    let owner = command.user_id()?;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}
