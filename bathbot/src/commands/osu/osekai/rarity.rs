use std::sync::Arc;

use bathbot_model::Rarity;
use bathbot_util::constants::OSEKAI_ISSUE;
use eyre::Result;

use crate::{
    active::{impls::MedalRarityPagination, ActiveMessages},
    core::ContextExt,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
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

    let pagination = MedalRarityPagination::builder()
        .ranking(ranking.into_boxed_slice())
        .msg_owner(command.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, &mut command)
        .await
}
