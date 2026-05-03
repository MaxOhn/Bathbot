use std::{borrow::Cow, collections::BTreeMap};

use bathbot_model::{
    ArchivedOsekaiRankingEntry, Countries, RankingEntries, RankingEntry, RankingKind,
};
use bathbot_util::{Authored, constants::GENERAL_ISSUE};
use eyre::{Report, Result};
use rkyv::vec::ArchivedVec;
use rosu_v2::prelude::Username;

use crate::{
    Context,
    active::{ActiveMessages, impls::RankingPagination},
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

pub(super) async fn count(
    command: InteractionCommand,
    country: Option<String>,
    ranking_kind: &str,
    ranking_options_kind: Option<&str>,
    data: RankingKind,
    value_fn: fn(&ArchivedOsekaiRankingEntry) -> u64,
) -> Result<()> {
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
    let ranking_fut = Context::redis().osekai_ranking(ranking_kind, ranking_options_kind);
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
        let archived_filter = |entry: &&ArchivedOsekaiRankingEntry| entry.country_code == code;

        prepare_amount_users(&ranking, archived_filter, value_fn)
    } else {
        prepare_amount_users(&ranking, |_| true, value_fn)
    };

    let entries = RankingEntries::Amount(entries);

    send_response(command, entries, data, name_res).await
}

pub(super) async fn pp(
    command: InteractionCommand,
    country: Option<String>,
    ranking_kind: &str,
    ranking_options_kind: Option<&str>,
    data: RankingKind,
    value_fn: fn(&ArchivedOsekaiRankingEntry) -> f32,
) -> Result<()> {
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
    let ranking_fut = Context::redis().osekai_ranking(ranking_kind, ranking_options_kind);
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
        let archived_filter = |entry: &&ArchivedOsekaiRankingEntry| entry.country_code == code;

        prepare_pp_users(&ranking, archived_filter, value_fn)
    } else {
        prepare_pp_users(&ranking, |_| true, value_fn)
    };

    let entries = RankingEntries::PpF32(entries);

    send_response(command, entries, data, name_res).await
}

fn prepare_amount_users(
    ranking: &ArchivedVec<ArchivedOsekaiRankingEntry>,
    archived_filter: impl Fn(&&ArchivedOsekaiRankingEntry) -> bool,
    value_fn: fn(&ArchivedOsekaiRankingEntry) -> u64,
) -> BTreeMap<usize, RankingEntry<u64>> {
    ranking
        .iter()
        .filter(archived_filter)
        .map(|entry| RankingEntry {
            value: value_fn(entry),
            name: entry.username.as_str().into(),
            country: Some(entry.country_code.as_str().into()),
        })
        .enumerate()
        .collect()
}

fn prepare_pp_users(
    ranking: &ArchivedVec<ArchivedOsekaiRankingEntry>,
    archived_filter: impl Fn(&&ArchivedOsekaiRankingEntry) -> bool,
    value_fn: fn(&ArchivedOsekaiRankingEntry) -> f32,
) -> BTreeMap<usize, RankingEntry<f32>> {
    ranking
        .iter()
        .filter(archived_filter)
        .map(|entry| RankingEntry {
            value: value_fn(entry),
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
