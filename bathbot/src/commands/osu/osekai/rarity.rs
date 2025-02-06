use bathbot_model::Rarity;
use bathbot_util::constants::GENERAL_ISSUE;
use eyre::{Report, Result};

use crate::{
    active::{impls::MedalRarityPagination, ActiveMessages},
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
    Context,
};

pub(super) async fn rarity(mut command: InteractionCommand) -> Result<()> {
    let ranking = match Context::redis().osekai_ranking::<Rarity>().await {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get cached rarity ranking"));
        }
    };

    let pagination = MedalRarityPagination::builder()
        .ranking(ranking)
        .msg_owner(command.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(&mut command)
        .await
}
