use super::UserValue;
use crate::{
    custom_client::{OsekaiRanking, OsekaiRankingEntry},
    embeds::{EmbedData, MedalRarityEmbed},
    util::constants::OSEKAI_ISSUE,
    util::MessageExt,
    BotResult, Context,
};

use std::{collections::BTreeMap, sync::Arc};
use twilight_model::application::interaction::ApplicationCommand;

pub(super) async fn count<R>(
    ctx: Arc<Context>,
    command: ApplicationCommand,
    kind: R,
) -> BotResult<()>
where
    R: OsekaiRanking<Entry = OsekaiRankingEntry<usize>>,
{
    let osekai_fut = ctx.clients.custom.get_osekai_ranking(kind);

    let ranking = match osekai_fut.await {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(why.into());
        }
    };

    let ranking: BTreeMap<_, _> = ranking
        .into_iter()
        .map(|entry| {
            let value = entry.value() as u64;
            let country = entry.country_code;
            let rank = entry.rank;
            let name = entry.username;

            (rank, (UserValue::Score(value), name, country))
        })
        .collect();

    // let embed_data = MedalRarityEmbed::new(&ranking, kind);
    // let builder = embed_data.into_builder().build().into();
    // let response = command.create_message(&ctx, builder).await?;

    // TODO: Pagination

    Ok(())
}

pub(super) async fn pp<R>(ctx: Arc<Context>, command: ApplicationCommand, kind: R) -> BotResult<()>
where
    R: OsekaiRanking<Entry = OsekaiRankingEntry<u32>>,
{
    let osekai_fut = ctx.clients.custom.get_osekai_ranking(kind);

    let ranking = match osekai_fut.await {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(why.into());
        }
    };

    let ranking: BTreeMap<_, _> = ranking
        .into_iter()
        .map(|entry| {
            let value = entry.value();
            let country = entry.country_code;
            let rank = entry.rank;
            let name = entry.username;

            (rank, (UserValue::Pp(value), name, country))
        })
        .collect();

    // let embed_data = MedalRarityEmbed::new(&ranking, kind);
    // let builder = embed_data.into_builder().build().into();
    // let response = command.create_message(&ctx, builder).await?;

    // TODO: Pagination

    Ok(())
}
