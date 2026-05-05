use std::num::NonZeroU8;

use bathbot_model::{OsekaiRankingEntries, OsekaiUserEntry};
use bathbot_util::{Authored, constants::GENERAL_ISSUE};
use eyre::{Report, Result};

use super::OsekaiMedalCount;
use crate::{
    Context,
    active::{ActiveMessages, impls::MedalCountPagination},
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

pub(super) async fn medal_count(
    mut command: InteractionCommand,
    args: OsekaiMedalCount,
) -> Result<()> {
    let owner = command.user_id()?;

    let osekai_fut = Context::redis().osekai_medal_count(args.country.as_deref(), NonZeroU8::MIN);

    let ranking = match osekai_fut.await {
        Ok(ranking) => ranking
            .try_deserialize::<OsekaiRankingEntries<OsekaiUserEntry>>()
            .unwrap(),
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get cached medal count ranking"));
        }
    };

    let total = ranking.max as usize;
    let ranking = ranking.data.into_iter().enumerate().collect();

    let pagination = MedalCountPagination::builder()
        .ranking(ranking)
        .total(total)
        .country(args.country)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(&mut command)
        .await
}
