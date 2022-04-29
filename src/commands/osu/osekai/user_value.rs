use std::{collections::BTreeMap, sync::Arc};

use eyre::Report;
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    custom_client::{OsekaiRanking, OsekaiRankingEntry},
    database::OsuData,
    embeds::{EmbedData, RankingEmbed, RankingEntry, RankingKindData},
    pagination::{Pagination, RankingPagination},
    util::{
        builder::MessageBuilder, constants::OSEKAI_ISSUE, numbers, ApplicationCommandExt, Authored,
    },
    BotResult, Context,
};

use super::UserValue;

pub(super) async fn count<R>(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()>
where
    R: OsekaiRanking<Entry = OsekaiRankingEntry<usize>>,
{
    let osekai_fut = ctx.client().get_osekai_ranking::<R>();
    let osu_fut = ctx.psql().get_user_osu(command.user_id()?);

    let (osekai_result, osu_result) = tokio::join!(osekai_fut, osu_fut);

    let ranking = match osekai_result {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
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
                value: UserValue::Amount(value),
                name,
                country: Some(country),
            };

            (i, entry)
        })
        .collect();

    let data = <R as OsekaiRanking>::RANKING;

    send_response(ctx, command, users, data, osu_result).await
}

pub(super) async fn pp<R>(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()>
where
    R: OsekaiRanking<Entry = OsekaiRankingEntry<u32>>,
{
    let owner = command.user_id()?;
    let osekai_fut = ctx.client().get_osekai_ranking::<R>();
    let osu_fut = ctx.psql().get_user_osu(owner);

    let (osekai_result, osu_result) = tokio::join!(osekai_fut, osu_fut);

    let ranking = match osekai_result {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
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
                value: UserValue::PpU32(value),
                name,
                country: Some(country),
            };

            (i, entry)
        })
        .collect();

    let data = <R as OsekaiRanking>::RANKING;

    send_response(ctx, command, users, data, osu_result).await
}

async fn send_response(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    users: BTreeMap<usize, RankingEntry>,
    data: RankingKindData,
    osu_result: BotResult<Option<OsuData>>,
) -> BotResult<()> {
    let username = match osu_result {
        Ok(osu) => osu.map(OsuData::into_username),
        Err(err) => {
            let report = Report::new(err).wrap_err("failed to retrieve user config");
            warn!("{:?}", report);

            None
        }
    };

    let author_idx = username
        .as_deref()
        .and_then(|name| users.values().position(|entry| entry.name == name));

    let total = users.len();
    let pages = numbers::div_euclid(20, total);
    let embed_data = RankingEmbed::new(&users, &data, author_idx, (1, pages));
    let embed = embed_data.build();
    let builder = MessageBuilder::new().embed(embed);
    let response = command.update(&ctx, &builder).await?.model().await?;

    // Pagination
    let pagination =
        RankingPagination::new(response, Arc::clone(&ctx), total, users, author_idx, data);

    pagination.start(ctx, command.user_id()?, 60);

    Ok(())
}
