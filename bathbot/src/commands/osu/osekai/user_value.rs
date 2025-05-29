use std::{borrow::Cow, collections::BTreeMap};

use bathbot_model::{
    ArchivedOsekaiRankingEntry, Countries, OsekaiRanking, OsekaiRankingEntry, RankingEntries,
    RankingEntry, RankingKind,
};
use bathbot_util::{Authored, constants::GENERAL_ISSUE};
use eyre::{Report, Result};
use rkyv::{
    Archive,
    bytecheck::CheckBytes,
    rancor::{Panic, Strategy},
    validation::{Validator, archive::ArchiveValidator},
    vec::ArchivedVec,
};
use rosu_v2::prelude::Username;

use crate::{
    Context,
    active::{ActiveMessages, impls::RankingPagination},
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

pub(super) async fn count<R>(command: InteractionCommand, country: Option<String>) -> Result<()>
where
    R: OsekaiRanking<Entry = OsekaiRankingEntry<usize>>,
    <R as OsekaiRanking>::Entry:
        for<'a> Archive<Archived: CheckBytes<Strategy<Validator<ArchiveValidator<'a>, ()>, Panic>>>,
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

                command.error(content).await?;

                return Ok(());
            }
        }
        None => None,
    };

    let owner = command.user_id()?;
    let ranking_fut = Context::redis().osekai_ranking::<R>();
    let name_fut = Context::user_config().osu_name(owner);

    let (osekai_res, name_res) = tokio::join!(ranking_fut, name_fut);

    let ranking = match osekai_res {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get cached osekai ranking"));
        }
    };

    let entries = if let Some(code) = country_code {
        let code = code.to_ascii_uppercase();
        let archived_filter =
            |entry: &&ArchivedOsekaiRankingEntry<usize>| entry.country_code == code;

        prepare_amount_users(&ranking, archived_filter)
    } else {
        prepare_amount_users(&ranking, |_| true)
    };

    let entries = RankingEntries::Amount(entries);
    let data = <R as OsekaiRanking>::RANKING;

    send_response(command, entries, data, name_res).await
}

pub(super) async fn pp<R>(command: InteractionCommand, country: Option<String>) -> Result<()>
where
    R: OsekaiRanking<Entry = OsekaiRankingEntry<u32>>,
    <R as OsekaiRanking>::Entry:
        for<'a> Archive<Archived: CheckBytes<Strategy<Validator<ArchiveValidator<'a>, ()>, Panic>>>,
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

                command.error(content).await?;

                return Ok(());
            }
        }
        None => None,
    };

    let owner = command.user_id()?;
    let ranking_fut = Context::redis().osekai_ranking::<R>();
    let name_fut = Context::user_config().osu_name(owner);

    let (osekai_res, name_res) = tokio::join!(ranking_fut, name_fut);

    let ranking = match osekai_res {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get cached osekai ranking"));
        }
    };

    let entries = if let Some(code) = country_code {
        let code = code.to_ascii_uppercase();
        let archived_filter = |entry: &&ArchivedOsekaiRankingEntry<u32>| entry.country_code == code;

        prepare_pp_users(&ranking, archived_filter)
    } else {
        prepare_pp_users(&ranking, |_| true)
    };

    let entries = RankingEntries::PpU32(entries);
    let data = <R as OsekaiRanking>::RANKING;

    send_response(command, entries, data, name_res).await
}

fn prepare_amount_users(
    ranking: &ArchivedVec<ArchivedOsekaiRankingEntry<usize>>,
    archived_filter: impl Fn(&&ArchivedOsekaiRankingEntry<usize>) -> bool,
) -> BTreeMap<usize, RankingEntry<u64>> {
    ranking
        .iter()
        .filter(archived_filter)
        .map(|entry| RankingEntry {
            value: entry.value().to_native() as u64,
            name: entry.username.as_str().into(),
            country: Some(entry.country_code.as_str().into()),
        })
        .enumerate()
        .collect()
}

fn prepare_pp_users(
    ranking: &ArchivedVec<ArchivedOsekaiRankingEntry<u32>>,
    archived_filter: impl Fn(&&ArchivedOsekaiRankingEntry<u32>) -> bool,
) -> BTreeMap<usize, RankingEntry<u32>> {
    ranking
        .iter()
        .filter(archived_filter)
        .map(|entry| RankingEntry {
            value: entry.value().to_native(),
            name: entry.username.as_str().into(),
            country: Some(entry.country_code.as_str().into()),
        })
        .enumerate()
        .collect()
}

async fn send_response(
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
        .begin(&mut command)
        .await
}
