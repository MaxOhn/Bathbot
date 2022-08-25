use std::sync::Arc;

use crate::{
    custom_client::Rarity,
    pagination::MedalRarityPagination,
    util::{constants::OSEKAI_ISSUE, interaction::InteractionCommand, InteractionCommandExt},
    BotResult, Context,
};

pub(super) async fn rarity(ctx: Arc<Context>, mut command: InteractionCommand) -> BotResult<()> {
    let ranking = match ctx.redis().osekai_ranking::<Rarity>().await {
        Ok(ranking) => ranking.to_inner(),
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    MedalRarityPagination::builder(ranking)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}
