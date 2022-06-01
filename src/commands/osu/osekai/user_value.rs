use std::{collections::BTreeMap, sync::Arc};

use eyre::Report;
use rkyv::{with::DeserializeWith, Deserialize, Infallible};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    core::commands::CommandOrigin,
    custom_client::UsernameWrapper,
    custom_client::{OsekaiRanking, OsekaiRankingEntry},
    database::OsuData,
    embeds::{RankingEntry, RankingKindData},
    pagination::RankingPagination,
    util::{constants::OSEKAI_ISSUE, ApplicationCommandExt, Authored},
    BotResult, Context,
};

use super::UserValue;

pub(super) async fn count<R>(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()>
where
    R: OsekaiRanking<Entry = OsekaiRankingEntry<usize>>,
{
    let redis = ctx.redis();
    let osekai_fut = redis.osekai_ranking::<R>();
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
        .get()
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let value = entry.value() as u64;
            let country = entry.country_code.deserialize(&mut Infallible).unwrap();

            let name = <UsernameWrapper as DeserializeWith<_, _, _>>::deserialize_with(
                &entry.username,
                &mut Infallible,
            )
            .unwrap();

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
    let redis = ctx.redis();
    let osekai_fut = redis.osekai_ranking::<R>();
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
        .get()
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let value = entry.value();
            let country = entry.country_code.deserialize(&mut Infallible).unwrap();

            let name = <UsernameWrapper as DeserializeWith<_, _, _>>::deserialize_with(
                &entry.username,
                &mut Infallible,
            )
            .unwrap();

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

    let builder = RankingPagination::builder(Arc::clone(&ctx), users, total, author_idx, data);

    builder
    .start_by_update()
        .start(ctx, CommandOrigin::Interaction { command })
        .await
}
