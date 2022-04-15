use std::sync::Arc;

use eyre::Report;
use rkyv::{Deserialize, Infallible};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    core::Context,
    custom_client::OsuTrackerMapperEntry,
    embeds::EmbedData,
    embeds::OsuTrackerMappersEmbed,
    pagination::{OsuTrackerMappersPagination, Pagination},
    util::{
        builder::MessageBuilder, constants::OSUTRACKER_ISSUE, numbers, ApplicationCommandExt,
        Authored,
    },
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

    counts.truncate(500);

    let pages = numbers::div_euclid(20, counts.len());
    let initial = &counts[..counts.len().min(20)];

    let embed = OsuTrackerMappersEmbed::new(initial, (1, pages)).into_builder();
    let builder = MessageBuilder::new().embed(embed.build());

    let response_raw = command.update(&ctx, &builder).await?;

    if counts.len() <= 20 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    let pagination = OsuTrackerMappersPagination::new(response, counts);
    let owner = command.user_id()?;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}
