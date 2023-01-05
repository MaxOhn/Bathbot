use std::sync::Arc;

use bathbot_model::Rarity;
use bathbot_util::constants::OSEKAI_ISSUE;
use eyre::Result;

use crate::{
    pagination::MedalRarityPagination,
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub(super) async fn rarity(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let ranking = match ctx.redis().osekai_ranking::<Rarity>().await {
        Ok(ranking) => ranking.into_original(),
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached rarity ranking"));
        }
    };

    MedalRarityPagination::builder(ranking)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}
