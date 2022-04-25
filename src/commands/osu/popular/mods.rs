use std::sync::Arc;

use rkyv::{Deserialize, Infallible};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    core::Context,
    custom_client::OsuTrackerModsEntry,
    embeds::EmbedData,
    embeds::OsuTrackerModsEmbed,
    pagination::{OsuTrackerModsPagination, Pagination},
    util::{
        builder::MessageBuilder, constants::OSUTRACKER_ISSUE, numbers, ApplicationCommandExt,
        Authored,
    },
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

    let embed = OsuTrackerModsEmbed::new(initial, (1, pages));
    let builder = MessageBuilder::new().embed(embed.build());

    let response_raw = command.update(&ctx, &builder).await?;

    if counts.len() <= 20 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    OsuTrackerModsPagination::new(response, counts).start(ctx, command.user_id()?, 60);

    Ok(())
}
