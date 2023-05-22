use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use bathbot_model::{
    ArchivedOsekaiRankingEntry, Countries, OsekaiRanking, OsekaiRankingEntry, RankingEntries,
    RankingEntry, RankingKind,
};
use bathbot_util::constants::OSEKAI_ISSUE;
use eyre::Result;
use rosu_v2::prelude::Username;

use crate::{
    active::{impls::RankingPagination, ActiveMessages},
    manager::redis::RedisData,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
    Context,
};

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
                Some(Cow::Owned(country))
            } else if let Some(code) = Countries::name(&country).to_code() {
                Some(code.into())
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
    let ranking_fut = ctx.redis().osekai_ranking::<R>();
    let name_fut = ctx.user_config().osu_name(owner);

    let (osekai_res, name_res) = tokio::join!(ranking_fut, name_fut);

    let ranking = match osekai_res {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached osekai ranking"));
        }
    };

    let entries = if let Some(code) = country_code {
        let code = code.to_ascii_uppercase();
        let original_filter = |entry: &&OsekaiRankingEntry<usize>| entry.country_code == code;
        let archived_filter =
            |entry: &&ArchivedOsekaiRankingEntry<usize>| entry.country_code == code;

        prepare_amount_users(&ranking, original_filter, archived_filter)
    } else {
        prepare_amount_users(&ranking, |_| true, |_| true)
    };

    let entries = RankingEntries::Amount(entries);
    let data = <R as OsekaiRanking>::RANKING;

    send_response(ctx, command, entries, data, name_res).await
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
                Some(Cow::Owned(country))
            } else if let Some(code) = Countries::name(&country).to_code() {
                Some(code.into())
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
    let ranking_fut = ctx.redis().osekai_ranking::<R>();
    let name_fut = ctx.user_config().osu_name(owner);

    let (osekai_res, name_res) = tokio::join!(ranking_fut, name_fut);

    let ranking = match osekai_res {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached osekai ranking"));
        }
    };

    let entries = if let Some(code) = country_code {
        let code = code.to_ascii_uppercase();
        let original_filter = |entry: &&OsekaiRankingEntry<u32>| entry.country_code == code;
        let archived_filter = |entry: &&ArchivedOsekaiRankingEntry<u32>| entry.country_code == code;

        prepare_pp_users(&ranking, original_filter, archived_filter)
    } else {
        prepare_pp_users(&ranking, |_| true, |_| true)
    };

    let entries = RankingEntries::PpU32(entries);
    let data = <R as OsekaiRanking>::RANKING;

    send_response(ctx, command, entries, data, name_res).await
}

fn prepare_amount_users(
    ranking: &RedisData<Vec<OsekaiRankingEntry<usize>>>,
    original_filter: impl Fn(&&OsekaiRankingEntry<usize>) -> bool,
    archived_filter: impl Fn(&&ArchivedOsekaiRankingEntry<usize>) -> bool,
) -> BTreeMap<usize, RankingEntry<u64>> {
    match ranking {
        RedisData::Original(ranking) => ranking
            .iter()
            .filter(original_filter)
            .map(|entry| RankingEntry {
                value: entry.value() as u64,
                name: entry.username.clone(),
                country: Some(entry.country_code.clone()),
            })
            .enumerate()
            .collect(),
        RedisData::Archive(ranking) => ranking
            .iter()
            .filter(archived_filter)
            .map(|entry| RankingEntry {
                value: entry.value() as u64,
                name: entry.username.as_str().into(),
                country: Some(entry.country_code.as_str().into()),
            })
            .enumerate()
            .collect(),
    }
}

fn prepare_pp_users(
    ranking: &RedisData<Vec<OsekaiRankingEntry<u32>>>,
    original_filter: impl Fn(&&OsekaiRankingEntry<u32>) -> bool,
    archived_filter: impl Fn(&&ArchivedOsekaiRankingEntry<u32>) -> bool,
) -> BTreeMap<usize, RankingEntry<u32>> {
    match ranking {
        RedisData::Original(ranking) => ranking
            .iter()
            .filter(original_filter)
            .map(|entry| RankingEntry {
                value: entry.value(),
                name: entry.username.clone(),
                country: Some(entry.country_code.clone()),
            })
            .enumerate()
            .collect(),
        RedisData::Archive(ranking) => ranking
            .iter()
            .filter(archived_filter)
            .map(|entry| RankingEntry {
                value: entry.value(),
                name: entry.username.as_str().into(),
                country: Some(entry.country_code.as_str().into()),
            })
            .enumerate()
            .collect(),
    }
}

async fn send_response(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    entries: RankingEntries,
    data: RankingKind,
    name_res: Result<Option<Username>>,
) -> Result<()> {
    let username = name_res.unwrap_or_else(|err| {
        warn!(?err, "Failed to get username");

        None
    });

    let author_idx = username.as_deref().and_then(|name| entries.name_pos(name));

    let total = entries.len();

    let pagination = RankingPagination::builder()
        .entries(entries)
        .total(total)
        .author_idx(author_idx)
        .kind(data)
        .defer(false)
        .msg_owner(command.user_id()?)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, &mut command)
        .await
}
