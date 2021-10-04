use crate::{
    custom_client::Rarity,
    embeds::{EmbedData, MedalRarityEmbed},
    pagination::{MedalRarityPagination, Pagination},
    util::{constants::OSEKAI_ISSUE, numbers, InteractionExt, MessageExt},
    BotResult, Context,
};

use std::sync::Arc;
use twilight_model::application::interaction::ApplicationCommand;

pub(super) async fn rarity(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    let osekai_fut = ctx.clients.custom.get_osekai_ranking(Rarity);

    let ranking = match osekai_fut.await {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(why.into());
        }
    };

    let pages = numbers::div_euclid(10, ranking.len());
    let embed_data = MedalRarityEmbed::new(&ranking[..10], 0, (1, pages));
    let builder = embed_data.into_builder().build().into();
    let response = command.create_message(&ctx, builder).await?.model().await?;
    let owner = command.user_id()?;
    let pagination = MedalRarityPagination::new(response, ranking);

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (medal rarity): {}")
        }
    });

    Ok(())
}
