use std::{collections::BTreeMap, sync::Arc};

use eyre::Result;
use rkyv::{with::DeserializeWith, Archived, Deserialize, Infallible};

use crate::{
    custom_client::UsernameWrapper,
    custom_client::{ArchivedOsekaiRankingEntry, OsekaiRanking, OsekaiRankingEntry},
    database::OsuData,
    embeds::{RankingEntry, RankingKindData},
    pagination::RankingPagination,
    util::{
        constants::OSEKAI_ISSUE, interaction::InteractionCommand, Authored, CountryCode,
        InteractionCommandExt,
    },
    Context,
};

use super::UserValue;

pub(super) async fn count<R>(
    ctx: Arc<Context>,
    command: InteractionCommand,
    country: Option<String>,
) -> Result<()>
where
    R: OsekaiRanking<Entry = OsekaiRankingEntry<usize>>,
{
    let country_code = match country {
        Some(country) => {
            if country.len() == 2 {
                Some(country.into())
            } else if let Some(code) = CountryCode::from_name(&country) {
                Some(code)
            } else {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                command.error(&ctx, content).await?;

                return Ok(());
            }
        }
        None => None,
    };

    let redis = ctx.redis();
    let osekai_fut = redis.osekai_ranking::<R>();
    let osu_fut = ctx.psql().get_user_osu(command.user_id()?);

    let (osekai_result, osu_result) = tokio::join!(osekai_fut, osu_fut);

    let ranking = match osekai_result {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached osekai ranking"));
        }
    };

    let users = if let Some(code) = country_code {
        let code = code.to_ascii_uppercase();
        let filter = |entry: &&ArchivedOsekaiRankingEntry<usize>| entry.country_code == code;

        prepare_amount_users(ranking.get(), filter)
    } else {
        prepare_amount_users(ranking.get(), |_| true)
    };

    let data = <R as OsekaiRanking>::RANKING;

    send_response(ctx, command, users, data, osu_result).await
}

pub(super) async fn pp<R>(
    ctx: Arc<Context>,
    command: InteractionCommand,
    country: Option<String>,
) -> Result<()>
where
    R: OsekaiRanking<Entry = OsekaiRankingEntry<u32>>,
{
    let country_code = match country {
        Some(country) => {
            if country.len() == 2 {
                Some(country.into())
            } else if let Some(code) = CountryCode::from_name(&country) {
                Some(code)
            } else {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                command.error(&ctx, content).await?;

                return Ok(());
            }
        }
        None => None,
    };

    let owner = command.user_id()?;
    let redis = ctx.redis();
    let osekai_fut = redis.osekai_ranking::<R>();
    let osu_fut = ctx.psql().get_user_osu(owner);

    let (osekai_result, osu_result) = tokio::join!(osekai_fut, osu_fut);

    let ranking = match osekai_result {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached osekai ranking"));
        }
    };

    let users = if let Some(code) = country_code {
        let code = code.to_ascii_uppercase();
        let filter = |entry: &&ArchivedOsekaiRankingEntry<u32>| entry.country_code == code;

        prepare_pp_users(ranking.get(), filter)
    } else {
        prepare_pp_users(ranking.get(), |_| true)
    };

    let data = <R as OsekaiRanking>::RANKING;

    send_response(ctx, command, users, data, osu_result).await
}

fn prepare_amount_users(
    ranking: &Archived<Vec<OsekaiRankingEntry<usize>>>,
    filter: impl Fn(&&ArchivedOsekaiRankingEntry<usize>) -> bool,
) -> BTreeMap<usize, RankingEntry> {
    ranking
        .iter()
        .filter(filter)
        .enumerate()
        .map(|(i, entry)| {
            let value = entry.value() as u64;
            let country = entry.country_code.deserialize(&mut Infallible).unwrap();

            let name = <UsernameWrapper as DeserializeWith<_, _, _>>::deserialize_with(
                &entry.username,
                &mut Infallible,
            );

            let entry = RankingEntry {
                value: UserValue::Amount(value),
                name: name.unwrap(),
                country: Some(country),
            };

            (i, entry)
        })
        .collect()
}

fn prepare_pp_users(
    ranking: &Archived<Vec<OsekaiRankingEntry<u32>>>,
    filter: impl Fn(&&ArchivedOsekaiRankingEntry<u32>) -> bool,
) -> BTreeMap<usize, RankingEntry> {
    ranking
        .iter()
        .filter(filter)
        .enumerate()
        .map(|(i, entry)| {
            let value = entry.value();
            let country = entry.country_code.deserialize(&mut Infallible).unwrap();

            let name = <UsernameWrapper as DeserializeWith<_, _, _>>::deserialize_with(
                &entry.username,
                &mut Infallible,
            );

            let entry = RankingEntry {
                value: UserValue::PpU32(value),
                name: name.unwrap(),
                country: Some(country),
            };

            (i, entry)
        })
        .collect()
}

async fn send_response(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    users: BTreeMap<usize, RankingEntry>,
    data: RankingKindData,
    osu_result: Result<Option<OsuData>>,
) -> Result<()> {
    let username = match osu_result {
        Ok(osu) => osu.map(OsuData::into_username),
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to get username"));

            None
        }
    };

    let author_idx = username
        .as_deref()
        .and_then(|name| users.values().position(|entry| entry.name == name));

    let total = users.len();

    let builder = RankingPagination::builder(users, total, author_idx, data);

    builder
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}
