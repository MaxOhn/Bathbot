use super::UserValue;
use crate::{
    custom_client::{OsekaiRanking, OsekaiRankingEntry},
    database::OsuData,
    embeds::{EmbedData, RankingEmbed, RankingEntry, RankingKindData},
    pagination::{Pagination, RankingPagination},
    util::{constants::OSEKAI_ISSUE, numbers, InteractionExt, MessageExt},
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
    let osu_fut = ctx.psql().get_user_osu(command.user_id()?);

    let (osekai_result, osu_result) = tokio::join!(osekai_fut, osu_fut);

    let ranking = match osekai_result {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(why.into());
        }
    };

    let users: BTreeMap<_, _> = ranking
        .into_iter()
        .enumerate()
        .map(|(i, entry)| {
            let value = entry.value() as u64;
            let country = entry.country_code;
            let name = entry.username;

            let entry = RankingEntry {
                value: UserValue::Score(value),
                name,
                country,
            };

            (i, entry)
        })
        .collect();

    let data = <R as OsekaiRanking>::RANKING;

    send_response(ctx, command, users, data, osu_result).await
}

pub(super) async fn pp<R>(ctx: Arc<Context>, command: ApplicationCommand, kind: R) -> BotResult<()>
where
    R: OsekaiRanking<Entry = OsekaiRankingEntry<u32>>,
{
    let owner = command.user_id()?;
    let osekai_fut = ctx.clients.custom.get_osekai_ranking(kind);
    let osu_fut = ctx.psql().get_user_osu(owner);

    let (osekai_result, osu_result) = tokio::join!(osekai_fut, osu_fut);

    let ranking = match osekai_result {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(why.into());
        }
    };

    let users: BTreeMap<_, _> = ranking
        .into_iter()
        .enumerate()
        .map(|(i, entry)| {
            let value = entry.value();
            let country = entry.country_code;
            let name = entry.username;

            let entry = RankingEntry {
                value: UserValue::Pp(value),
                name,
                country,
            };

            (i, entry)
        })
        .collect();

    let data = <R as OsekaiRanking>::RANKING;

    send_response(ctx, command, users, data, osu_result).await
}

async fn send_response(
    ctx: Arc<Context>,
    command: ApplicationCommand,
    users: BTreeMap<usize, RankingEntry>,
    data: RankingKindData,
    osu_result: BotResult<Option<OsuData>>,
) -> BotResult<()> {
    let username = match osu_result {
        Ok(osu) => osu.map(OsuData::into_username),
        Err(why) => {
            unwind_error!(warn, why, "Failed to retrieve user config: {}");

            None
        }
    };

    let author_idx = username
        .as_deref()
        .and_then(|name| users.values().position(|entry| entry.name == name));

    let total = users.len();
    let pages = numbers::div_euclid(20, total);
    let embed_data = RankingEmbed::new(&users, &data, author_idx, (1, pages));
    let builder = embed_data.into_builder().build().into();
    let response = command.create_message(&ctx, builder).await?.model().await?;

    // Pagination
    let pagination =
        RankingPagination::new(response, Arc::clone(&ctx), total, users, author_idx, data);

    let owner = command.user_id()?;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (ranking): {}")
        }
    });

    Ok(())
}
