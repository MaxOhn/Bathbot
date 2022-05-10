use std::sync::Arc;

use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    custom_client::Rarity,
    embeds::{EmbedData, MedalRarityEmbed},
    pagination::{MedalRarityPagination, Pagination},
    util::{
        builder::MessageBuilder, constants::OSEKAI_ISSUE, numbers, ApplicationCommandExt, Authored,
    },
    BotResult, Context,
};

pub(super) async fn rarity(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    let ranking = match ctx.redis().osekai_ranking::<Rarity>().await {
        Ok(ranking) => ranking.to_inner(),
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let pages = numbers::div_euclid(10, ranking.len());
    let embed_data = MedalRarityEmbed::new(&ranking[..10], 0, (1, pages));
    let embed = embed_data.build();
    let builder = MessageBuilder::new().embed(embed);
    let response = command.update(&ctx, &builder).await?.model().await?;

    MedalRarityPagination::new(response, ranking).start(ctx, command.user_id()?, 60);

    Ok(())
}
