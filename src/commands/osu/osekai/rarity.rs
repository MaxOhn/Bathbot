use std::sync::Arc;

use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    core::commands::CommandOrigin,
    custom_client::Rarity,
    pagination::MedalRarityPagination,
    util::{constants::OSEKAI_ISSUE, ApplicationCommandExt},
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

    MedalRarityPagination::builder(ranking)
    .start_by_update()
        .start(ctx, CommandOrigin::Interaction { command })
        .await
}
