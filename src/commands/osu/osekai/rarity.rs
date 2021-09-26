use crate::{
    custom_client::Rarity,
    embeds::{EmbedData, MedalRarityEmbed},
    util::constants::OSEKAI_ISSUE,
    util::MessageExt,
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

    let embed_data = MedalRarityEmbed::new(&ranking[..10], 0);
    let builder = embed_data.into_builder().build().into();
    let response = command.create_message(&ctx, builder).await?;

    // TODO: Pagination

    Ok(())
}
