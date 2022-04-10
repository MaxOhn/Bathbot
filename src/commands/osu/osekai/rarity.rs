use std::sync::Arc;

use eyre::Report;
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    custom_client::Rarity,
    embeds::{EmbedData, MedalRarityEmbed},
    pagination::{MedalRarityPagination, Pagination},
    util::{constants::OSEKAI_ISSUE, numbers},
    BotResult, Context,
};

pub(super) async fn rarity(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    let osekai_fut = ctx.clients.custom.get_osekai_ranking::<Rarity>();

    let ranking = match osekai_fut.await {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let pages = numbers::div_euclid(10, ranking.len());
    let embed_data = MedalRarityEmbed::new(&ranking[..10], 0, (1, pages));
    let embed = embed_data.into_builder().build();
    let builder = MessageBuilder::new().embed(embed);
    let response = command.update(&ctx, &builder).await?.model().await?;
    let owner = command.user_id()?;
    let pagination = MedalRarityPagination::new(response, ranking);

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}
